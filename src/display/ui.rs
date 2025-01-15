use crossbeam::channel::Sender;
use cursive::{
    traits::{Nameable, Resizable, Scrollable},
    view::ScrollStrategy,
    views::{Dialog, EditView, LinearLayout, NamedView, Panel, SelectView},
    Cursive,
};

use crate::display::DisplayCommand;
use crate::network::utils::usable_interfaces;

fn create_file_input(display_tx: &Sender<DisplayCommand>) -> Panel<NamedView<EditView>> {
    Panel::new(
        EditView::new()
            .on_submit({
                let tx = display_tx.clone();
                move |siv, name: &str| {
                    siv.call_on_name("file_input", |field: &mut EditView| field.set_content(""));
                    tx.send(DisplayCommand::AdvertiseFile(name.to_string()))
                        .unwrap()
                }
            })
            .with_name("file_input"),
    )
    .title("file path")
}

fn create_interface_select(display_tx: &Sender<DisplayCommand>) -> Panel<SelectView> {
    Panel::new(
        SelectView::new()
            .with_all(
                usable_interfaces()
                    .into_iter()
                    .map(|i| (format!("{0}: {1}", i.name, i.mac.unwrap()), i.name)),
            )
            .on_submit({
                let tx = display_tx.clone();
                move |_, name: &String| {
                    tx.send(DisplayCommand::ChangeInterface(name.to_string()))
                        .unwrap()
                }
            }),
    )
}

pub fn start_ui(siv: &mut Cursive, display_tx: Sender<DisplayCommand>) {
    siv.add_fullscreen_layer(
        LinearLayout::horizontal()
            .child(
                LinearLayout::vertical()
                    .child(
                        Dialog::around(create_file_input(&display_tx))
                            .title("send a file over arp")
                            .full_width(),
                    )
                    .child(
                        create_interface_select(&display_tx)
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
