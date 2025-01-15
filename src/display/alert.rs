use cursive::{
    views::{Dialog, TextView},
    Cursive, CursiveRunner,
};

pub fn alert_user(siv: &mut CursiveRunner<&mut Cursive>, message: String) {
    siv.add_layer(
        Dialog::new()
            .content(TextView::new(message))
            .button("ok", |s| {
                s.pop_layer();
                return;
            }),
    );
}
