use crossbeam::channel::Sender;
use cursive::{
    views::{Dialog, EditView},
    Cursive,
};

use crate::display::NetworkCommand;

pub fn change_path(siv: &mut Cursive, network_tx: &Sender<NetworkCommand>) {
    siv.add_layer(
        Dialog::around(EditView::new().on_submit({
            let tx = network_tx.clone();
            move |siv, name: &str| {
                tx.send(NetworkCommand::UpdateLocalPath(name.to_string()))
                    .expect("Error: unable to update path.");

                siv.pop_layer();
            }
        }))
        .title("Enter path to store files"),
    );
}
