use crate::network::usable_interfaces;
use crossbeam::channel::{unbounded, Receiver, Sender};
use cursive::{
    traits::{Nameable, Resizable, Scrollable},
    view::ScrollStrategy,
    views::{Dialog, EditView, LinearLayout, Panel, SelectView, TextView},
};
use std::thread;

enum DisplayCommand {
    NewFile(String),
    FetchFile(String),
    ChangeInterface(String),
}

enum NetworkCommand {
    AdvertiseFile(String),
    SendFile(String),
}

fn network_thread(display_tx: Sender<DisplayCommand>, network_rx: Receiver<NetworkCommand>) {
    // send display updates and receive network updates
}

pub fn run() {
    let (display_tx, display_rx) = unbounded::<DisplayCommand>();
    let (network_tx, network_rx) = unbounded::<NetworkCommand>();

    thread::spawn(move || network_thread(display_tx.clone(), network_rx));

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
                            Panel::new(EditView::new())
                                .title("file path")
                                .with_name("file_input"),
                        )
                        .title("send a file over arp")
                        .full_width(),
                    )
                    .child(
                        Dialog::around(
                            SelectView::new()
                                .with_all(
                                    usable_interfaces().into_iter().map(|i| {
                                        (format!("{0}: {1}", i.name, i.mac.unwrap()), i.name)
                                    }),
                                )
                                .on_submit(|siv, name: &String| ()),
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

    siv.run();
}
