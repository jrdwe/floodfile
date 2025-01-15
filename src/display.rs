use crossbeam::channel::unbounded;
use cursive::views::{Dialog, LinearLayout, TextView};
use std::fs;
use std::thread;
use ui::start_ui;

use crate::display::alert::alert_user;
use crate::display::network_thread::network_thread;

pub mod alert;
pub mod network_thread;
pub mod path;
pub mod ui;

pub enum DisplayCommand {
    AdvertiseFile(String),
    NewFile(String),
    ChangeInterface(String),
    AlertUser(String),
}

pub enum NetworkCommand {
    UpdateLocalPath(String),
    AdvertiseFile(String),
    RequestFile(String),
    ChangeInterface(String),
}

pub fn start() {
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
        move |siv| path::change_path(siv, &tx)
    });

    siv.set_autohide_menu(false);
    start_ui(&mut siv, display_tx.clone());

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
                    let n_tx = network_tx.clone();
                    let d_tx = display_tx.clone();
                    siv.call_on_name("file_list", move |file_list: &mut LinearLayout| {
                        let available =
                            Dialog::around(TextView::new(&file)).button("download", move |_s| {
                                d_tx.send(DisplayCommand::AlertUser(String::from("downloading..")))
                                    .unwrap();
                                n_tx.send(NetworkCommand::RequestFile(file.clone()))
                                    .unwrap();
                            });

                        file_list.add_child(available);
                    });
                }
                DisplayCommand::ChangeInterface(interface) => {
                    network_tx
                        .send(NetworkCommand::ChangeInterface(interface.clone()))
                        .unwrap();

                    siv.call_on_name("file_list", |file_list: &mut LinearLayout| {
                        file_list.clear();
                    });
                }
                DisplayCommand::AlertUser(message) => {
                    alert_user(&mut siv, message);
                }
            }
        }

        siv.step();
    }
}
