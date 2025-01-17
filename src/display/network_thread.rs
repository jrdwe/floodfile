use crossbeam::channel::{Receiver, Sender};
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;

use crate::display::config::Config;
use crate::display::{DisplayCommand, NetworkCommand};
use crate::network::payload::Payload;
use crate::network::utils::{compute_filehash, usable_interfaces};

pub fn network_thread(display_tx: Sender<DisplayCommand>, network_rx: Receiver<NetworkCommand>) {
    let mut cfg = Config::new();

    loop {
        while let Ok(command) = network_rx.try_recv() {
            match command {
                NetworkCommand::AdvertiseFile(filepath) => {
                    let hash = match compute_filehash(&filepath) {
                        Ok(hash) => hash,
                        _ => continue,
                    };

                    cfg.sharing.insert(hash, filepath.clone());
                    match cfg.channel.send(Payload::Advertise(filepath)) {
                        Ok(_) => (),
                        Err(e) => send_alert(&display_tx, e.to_string()),
                    };

                    send_alert(&display_tx, String::from("sharing!"));
                }
                NetworkCommand::RequestFile(file) => {
                    let hash = match compute_filehash(&file) {
                        Ok(hash) => hash,
                        Err(e) => {
                            send_alert(&display_tx, e.to_string());
                            continue;
                        }
                    };

                    cfg.requested.insert(hash);
                    match cfg.channel.send(Payload::DownloadRequest(hash)) {
                        Ok(_) => (),
                        Err(e) => send_alert(&display_tx, e.to_string()),
                    };
                }
                NetworkCommand::ChangeInterface(name) => {
                    if cfg.channel.interface_name() == name {
                        continue;
                    }

                    let interface = usable_interfaces()
                        .into_iter()
                        .find(|i| i.name == name)
                        .unwrap();

                    cfg = Config::from(interface);
                }
                NetworkCommand::UpdateLocalPath(path) => {
                    if !Path::new(&path).is_dir() {
                        send_alert(&display_tx, String::from("invalid path"));
                        continue;
                    };

                    match cfg.channel.set_path(&path) {
                        Ok(_) => (),
                        Err(e) => send_alert(&display_tx, e.to_string()),
                    };
                }
            }
        }

        let packet = cfg.channel.recv();
        if let Ok(Some(packet)) = packet {
            match packet {
                Payload::File(filehash, data) => {
                    if !cfg.requested.contains(&filehash) {
                        continue;
                    }

                    cfg.requested.remove(&filehash);
                    if let Some(filename) = cfg.shared.get(&filehash) {
                        // extrace only the filename
                        let filepath = PathBuf::from(filename);
                        let filename = filepath.file_name().unwrap();

                        // destination path + filename
                        let mut path = cfg.channel.get_path();
                        path.push(filename);

                        // write file to disk!
                        let mut file = File::create(&path).unwrap();
                        file.write_all(&data).unwrap();

                        // notify user!
                        send_alert(&display_tx, format!("saved: {0}", path.to_str().unwrap()))
                    }
                }
                Payload::Advertise(filepath) => {
                    let hash = match compute_filehash(&filepath) {
                        Ok(hash) => hash,
                        Err(_) => continue,
                    };

                    if cfg.shared.get(&hash).is_some() || cfg.sharing.get(&hash).is_some() {
                        continue;
                    }

                    cfg.shared.insert(hash, filepath.clone());
                    display_tx.send(DisplayCommand::NewFile(filepath)).unwrap()
                }
                Payload::DownloadRequest(hash) => {
                    if let Some(file) = cfg.sharing.get(&hash) {
                        let data: Vec<u8> = fs::read(&file).unwrap();
                        match cfg.channel.send(Payload::File(hash, data)) {
                            Ok(_) => (),
                            Err(e) => send_alert(&display_tx, e.to_string()),
                        };
                    }
                }
            }
        }
    }
}

fn send_alert(tx: &Sender<DisplayCommand>, msg: String) {
    tx.send(DisplayCommand::AlertUser(msg)).unwrap()
}
