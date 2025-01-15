use crossbeam::channel::{Receiver, Sender};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::path::PathBuf;

use crate::display::{DisplayCommand, NetworkCommand};
use crate::network::payload::Payload;
use crate::network::utils::{compute_filehash, usable_interfaces};
use crate::network::{Channel, FileHash};

fn send_alert(tx: &Sender<DisplayCommand>, msg: String) {
    tx.send(DisplayCommand::AlertUser(msg)).unwrap()
}

pub fn network_thread(display_tx: Sender<DisplayCommand>, network_rx: Receiver<NetworkCommand>) {
    let interfaces = usable_interfaces();
    let mut channel = Channel::new(interfaces[0].clone()).unwrap();
    let mut shared: HashMap<FileHash, String> = HashMap::new();
    let mut sharing: HashMap<FileHash, String> = HashMap::new();

    loop {
        while let Ok(command) = network_rx.try_recv() {
            match command {
                NetworkCommand::AdvertiseFile(filepath) => {
                    let hash = match compute_filehash(&filepath) {
                        Ok(hash) => hash,
                        _ => continue,
                    };

                    sharing.insert(hash, filepath.clone());
                    match channel.send(Payload::Advertise(filepath)) {
                        Ok(_) => (),
                        Err(e) => send_alert(&display_tx, e.to_string()),
                    };
                }
                NetworkCommand::RequestFile(file) => {
                    let hash = match compute_filehash(&file) {
                        Ok(hash) => hash,
                        Err(e) => {
                            send_alert(&display_tx, e.to_string());
                            continue;
                        }
                    };

                    match channel.send(Payload::Download(hash)) {
                        Ok(_) => (),
                        Err(e) => send_alert(&display_tx, e.to_string()),
                    };
                }
                NetworkCommand::ChangeInterface(name) => {
                    if channel.interface_name() == name {
                        continue;
                    }

                    let interface = usable_interfaces()
                        .into_iter()
                        .find(|i| i.name == name)
                        .unwrap();

                    channel = Channel::new(interface).unwrap();
                }
                NetworkCommand::UpdateLocalPath(path) => {
                    if !Path::new(&path).is_dir() {
                        send_alert(&display_tx, String::from("invalid path"));
                        continue;
                    };

                    match channel.set_path(&path) {
                        Ok(_) => (),
                        Err(e) => send_alert(&display_tx, e.to_string()),
                    };
                }
            }
        }

        let packet = channel.recv();
        if let Ok(Some(packet)) = packet {
            match packet {
                Payload::File(filehash, data) => {
                    if let Some(filename) = shared.get(&filehash) {
                        // extrace only the filename
                        let filepath = PathBuf::from(filename);
                        let filename = filepath.file_name().unwrap();

                        // destination path + filename
                        let mut path = channel.get_path();
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

                    if shared.get(&hash).is_some() {
                        continue;
                    }

                    shared.insert(hash, filepath.clone());
                    display_tx.send(DisplayCommand::NewFile(filepath)).unwrap()
                }
                Payload::Download(hash) => {
                    if let Some(file) = sharing.get(&hash) {
                        let data: Vec<u8> = fs::read(&file).unwrap();
                        match channel.send(Payload::File(hash, data)) {
                            Ok(_) => (),
                            Err(e) => send_alert(&display_tx, e.to_string()),
                        };
                    }
                }
            }
        }
    }
}
