use crate::network::{compute_filehash, usable_interfaces, Channel, FileHash, Payload};
use crossbeam::channel::{unbounded, Receiver, Sender};
use cursive::{
    traits::{Nameable, Resizable, Scrollable},
    view::ScrollStrategy,
    views::{Dialog, EditView, LinearLayout, Panel, SelectView, TextView},
    Cursive,
};
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::thread;

enum DisplayCommand {
    AdvertiseFile(String),
    NewFile(String),
    ChangeInterface(String),
}

enum NetworkCommand {
    UpdateLocalPath(String),
    AdvertiseFile(String),
    RequestFile(String),
    ChangeInterface(String),
}

fn network_thread(display_tx: Sender<DisplayCommand>, network_rx: Receiver<NetworkCommand>) {
    let interfaces = usable_interfaces();
    let mut channel = Channel::new(interfaces[0].clone());
    let mut shared: HashMap<FileHash, String> = HashMap::new();
    let mut sharing: HashMap<FileHash, String> = HashMap::new();

    loop {
        while let Ok(command) = network_rx.try_recv() {
            match command {
                NetworkCommand::AdvertiseFile(filepath) => {
                    let hash = compute_filehash(&filepath);
                    sharing.insert(hash, filepath.clone());
                    channel.send(Payload::Advertise(filepath));
                }
                NetworkCommand::RequestFile(file) => {
                    let hash = compute_filehash(&file);
                    channel.send(Payload::Download(hash));
                }
                NetworkCommand::ChangeInterface(name) => {
                    if channel.interface_name() == name {
                        continue;
                    }

                    let interface = usable_interfaces()
                        .into_iter()
                        .find(|i| i.name == name)
                        .unwrap();

                    channel = Channel::new(interface);
                }
                NetworkCommand::UpdateLocalPath(mut path) => {
                    if path.chars().last().unwrap() != '/' {
                        path = path.clone() + "/";
                    }

                    channel.set_path(&path);
                }
            }
        }

        let packet = channel.recv();
        if let Some(packet) = packet {
            match packet {
                Payload::File(filehash, data) => {
                    if let Some(filename) = shared.get(&filehash) {
                        let filename = filename.split("/").last().unwrap();
                        let mut file = File::create(channel.get_path() + filename).unwrap();
                        file.write_all(&data).unwrap();
                    }
                }
                Payload::Advertise(filepath) => {
                    let hash = compute_filehash(&filepath);
                    shared.insert(hash, filepath.clone());
                    display_tx.send(DisplayCommand::NewFile(filepath)).unwrap()
                }
                Payload::Download(hash) => {
                    if let Some(file) = sharing.get(&hash) {
                        let data: Vec<u8> = fs::read(&file).unwrap();
                        channel.send(Payload::File(hash, data));
                    }
                }
            }
        }
    }
}

fn start_interface(siv: &mut Cursive, display_tx: Sender<DisplayCommand>) {
    siv.add_fullscreen_layer(
        LinearLayout::horizontal()
            .child(
                LinearLayout::vertical()
                    .child(
                        Dialog::around(
                            Panel::new(
                                EditView::new()
                                    .on_submit({
                                        let tx = display_tx.clone();
                                        move |siv, name: &str| {
                                            siv.call_on_name(
                                                "file_input",
                                                |field: &mut EditView| field.set_content(""),
                                            );

                                            tx.send(DisplayCommand::AdvertiseFile(name.to_string()))
                                                .unwrap()
                                        }
                                    })
                                    .with_name("file_input"),
                            )
                            .title("file path"),
                        )
                        .title("send a file over arp")
                        .full_width(),
                    )
                    .child(
                        Panel::new(
                            SelectView::new()
                                .with_all(
                                    usable_interfaces().into_iter().map(|i| {
                                        (format!("{0}: {1}", i.name, i.mac.unwrap()), i.name)
                                    }),
                                )
                                .on_submit({
                                    let tx = display_tx.clone();
                                    move |_, name: &String| {
                                        tx.send(DisplayCommand::ChangeInterface(name.to_string()))
                                            .unwrap()
                                    }
                                }),
                        )
                        .title("interfaces")
                        .full_height(),
                    ),
            )
            .child(
                Panel::new(
                    LinearLayout::vertical()
                        .with_name("file_list")
                        .full_height()
                        .full_width()
                        .scrollable()
                        .scroll_strategy(ScrollStrategy::StickToBottom),
                )
                .title("available files"),
            ),
    );
}

fn change_path(siv: &mut Cursive, network_tx: &Sender<NetworkCommand>) {
    siv.add_layer(
        Dialog::around(EditView::new().on_submit({
            let tx = network_tx.clone();
            move |siv, name: &str| {
                // TODO: display error on invalid dir
                if Path::new(&name).is_dir() {
                    tx.send(NetworkCommand::UpdateLocalPath(name.to_string()))
                        .unwrap();
                }

                siv.pop_layer();
            }
        }))
        .title("Enter path to store files"),
    );
}

pub fn run() {
    let (display_tx, display_rx) = unbounded::<DisplayCommand>();
    let (network_tx, network_rx) = unbounded::<NetworkCommand>();

    thread::spawn({
        let display_tx = display_tx.clone();
        move || network_thread(display_tx, network_rx)
    });

    let mut siv = cursive::default();
    siv.load_toml(include_str!("../assets/theme.toml")).unwrap();

    siv.menubar().add_leaf("quit", |siv| siv.quit());
    siv.menubar().add_leaf("storage-path", {
        let tx = network_tx.clone();
        move |siv| change_path(siv, &tx)
    });

    siv.set_autohide_menu(false);
    start_interface(&mut siv, display_tx.clone());

    let mut siv = siv.runner();
    while siv.is_running() {
        siv.refresh();
        while let Ok(command) = display_rx.try_recv() {
            match command {
                DisplayCommand::AdvertiseFile(file) => {
                    if !fs::exists(&file).unwrap() {
                        continue;
                    }

                    network_tx
                        .send(NetworkCommand::AdvertiseFile(file.clone()))
                        .unwrap();
                }
                DisplayCommand::NewFile(file) => {
                    let tx = network_tx.clone();
                    siv.call_on_name("file_list", move |file_list: &mut LinearLayout| {
                        let available = Dialog::around(TextView::new(&file))
                            .button("download", move |_s| {
                                tx.send(NetworkCommand::RequestFile(file.clone())).unwrap()
                            });

                        file_list.add_child(available);
                    });
                }
                DisplayCommand::ChangeInterface(interface) => {
                    network_tx
                        .send(NetworkCommand::ChangeInterface(interface.clone()))
                        .unwrap();

                    // TODO: only if successful - need error handling here
                    siv.call_on_name("file_list", |file_list: &mut LinearLayout| {
                        file_list.clear();
                    });
                }
            }
        }

        siv.step();
    }
}
