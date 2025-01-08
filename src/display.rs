use crate::network::{usable_interfaces, Channel};
use crossbeam::channel::{unbounded, Receiver, Sender};
use cursive::{
    traits::{Nameable, Resizable, Scrollable},
    view::ScrollStrategy,
    views::{Button, Dialog, EditView, LinearLayout, Panel, SelectView, TextView},
    Cursive,
};
use std::fs;
use std::path::Path;
use std::thread;

enum DisplayCommand {
    NewFile(String),
    FetchFile(String),
    ChangeInterface(String),
}

enum NetworkCommand {
    UpdateLocalPath(String),
    AdvertiseFile(String),
    SendFile(String),
    ChangeInterface(String),
}

fn network_thread(display_tx: Sender<DisplayCommand>, network_rx: Receiver<NetworkCommand>) {
    let interfaces = usable_interfaces();
    let mut channel = Channel::new(interfaces[0].clone());

    loop {
        while let Ok(command) = network_rx.try_recv() {
            match command {
                NetworkCommand::AdvertiseFile(filepath) => {
                    // TODO: append information on advertising to sharing
                    println!("filepath: {}", filepath);
                }
                NetworkCommand::SendFile(file) => {
                    // TODO: sends a small file
                    // channel.send(file);
                }
                NetworkCommand::ChangeInterface(name) => {
                    if channel.interface_name() == name {
                        continue;
                    }

                    let interface = usable_interfaces()
                        .into_iter()
                        .find(|i| i.name == name)
                        .unwrap();

                    eprintln!("changed interface: {}", interface.name);
                    channel = Channel::new(interface);
                }
                NetworkCommand::UpdateLocalPath(path) => {
                    // TODO: check it's valid
                    channel.set_path(&path);
                }
            }
        }

        channel.recv();
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

                                            tx.send(DisplayCommand::NewFile(name.to_string()))
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
                    eprintln!("updating local path");
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
                DisplayCommand::NewFile(file) => {
                    if !fs::exists(&file).unwrap() {
                        continue;
                    }

                    let tx = network_tx.clone();
                    siv.call_on_name("file_list", move |file_list: &mut LinearLayout| {
                        let available = Dialog::around(TextView::new(&file))
                            .button("download", move |s| {
                                tx.send(NetworkCommand::SendFile(file.clone())).unwrap()
                            });

                        file_list.add_child(available);
                    });
                }
                DisplayCommand::ChangeInterface(interface) => {
                    network_tx
                        .send(NetworkCommand::ChangeInterface(interface.clone()))
                        .unwrap();

                    // TODO: only if successful? need error handling here
                    siv.call_on_name("file_list", |file_list: &mut LinearLayout| {
                        file_list.clear();
                    });
                }
                DisplayCommand::FetchFile(id) => {
                    // TODO: poll for existing file
                    eprintln!("id: {}", id);
                }
            }
        }

        siv.step();
    }
}
