use cursive::{
    traits::{Nameable, Resizable, Scrollable},
    view::ScrollStrategy,
    views::{Dialog, EditView, LinearLayout, Panel, TextView},
};

enum DisplayCommand {
    NewFile(String),
    FetchFile(String),
    ChangeInterface(String),
}

enum NetworkCommand {
    AdvertiseFile(String),
    SendFile(String),
}

pub fn run() {
    let mut siv = cursive::default();
    siv.set_autohide_menu(false);
    siv.menubar().add_leaf("quit", |siv| siv.quit());
    siv.load_toml(include_str!("../assets/theme.toml")).unwrap();

    siv.add_fullscreen_layer(
        LinearLayout::horizontal()
            .child(
                Dialog::around(
                    Panel::new(EditView::new())
                        .title("file path")
                        .fixed_height(3)
                        .with_name("file_input"),
                )
                .title("send a file over arp")
                .full_width(),
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
