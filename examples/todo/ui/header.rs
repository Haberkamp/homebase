use std::path::PathBuf;

use gpui::{div, prelude::*, rgb};

pub fn db_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/todo/todos.db")
}

pub fn migrations_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/todo/migrations")
}

pub fn header() -> impl IntoElement {
    let db = db_path();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xl()
                .text_color(rgb(0xe0e0e0))
                .child("Homebase todos"),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x888888))
                .child(format!("{}", db.display())),
        )
}
