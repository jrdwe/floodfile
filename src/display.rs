use crate::network::{usable_interfaces, Channel};
use crossbeam::channel::{unbounded, Receiver, Sender};
use cursive::{
    traits::{Nameable, Resizable, Scrollable},
    view::ScrollStrategy,
    views::{Button, Dialog, EditView, LinearLayout, Panel, SelectView, TextView},
};
use pnet::datalink::NetworkInterface;
use std::thread;

enum DisplayCommand {
    NewFile(String),
    FetchFile(String),
}

enum NetworkCommand {
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
                    println!("filepath: {}", filepath);
                }
                NetworkCommand::SendFile(id) => {
                    println!("id: {}", id);
                    // grab from disk
                    // spam over network!
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
            }
        }
    }

    // needs to set a channel -> define in networking
    // send display updates and receive network updates
}

pub fn run() {
    let (display_tx, display_rx) = unbounded::<DisplayCommand>();
    let (network_tx, network_rx) = unbounded::<NetworkCommand>();

    thread::spawn({
        // requires that move takes no ownership of display_tx
        let display_tx = display_tx.clone();
        move || network_thread(display_tx, network_rx)
    });

    let mut siv = cursive::default();
    siv.set_autohide_menu(false);

    // build channels for networking + display handling
    // improved error handling
    // show interface options?

    siv.menubar().add_leaf("quit", |siv| siv.quit());
    siv.load_toml(include_str!("../assets/theme.toml")).unwrap();

    siv.add_fullscreen_layer(
        LinearLayout::horizontal()
            .child(
                LinearLayout::vertical()
                    .child(
                        Dialog::around(
                            Panel::new(
                                EditView::new()
                                    .on_submit(move |siv, name: &str| {
                                        siv.call_on_name("file_input", |field: &mut EditView| {
                                            field.set_content("")
                                        });

                                        // should it strip path information? security is not a concern!
                                        display_tx
                                            .send(DisplayCommand::NewFile(name.to_string()))
                                            .unwrap()
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
                                .on_submit(move |_siv, name: &String| ()),
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

    let mut siv = siv.runner();
    while siv.is_running() {
        siv.refresh();
        while let Ok(command) = display_rx.try_recv() {
            match command {
                DisplayCommand::NewFile(file) => {
                    siv.call_on_name("file_list", |file_list: &mut LinearLayout| {
                        let available =
                            Dialog::around(TextView::new(file)).button("download", |s| s.quit());

                        file_list.add_child(available);
                    });
                }
                DisplayCommand::FetchFile(id) => {
                    println!("id: {}", id);
                }
            }
        }

        siv.step();
    }

    // loop over ui command here!
}
