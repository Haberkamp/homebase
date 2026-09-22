use gpui::{App, MouseButton, MouseUpEvent, Window, div, prelude::*, rgb};

use crate::domain::Todo;

pub fn todo_row(
    todo: Todo,
    on_toggle: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
    on_delete: impl Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let completed = todo.completed;
    let text = todo.text;

    div()
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .gap_3()
        .bg(rgb(0x2a2a2a))
        .border_1()
        .border_color(rgb(0x444444))
        .rounded_lg()
        .p_3()
        .child(
            div()
                .flex()
                .flex_1()
                .cursor_pointer()
                .text_color(if completed {
                    rgb(0x888888)
                } else {
                    rgb(0xe0e0e0)
                })
                .child(format!("{} {text}", if completed { "[x]" } else { "[ ]" }))
                .on_mouse_up(MouseButton::Left, on_toggle),
        )
        .child(
            div()
                .bg(rgb(0x7c4a4a))
                .hover(|style| style.bg(rgb(0x9c5a5a)).cursor_pointer())
                .rounded_lg()
                .px_3()
                .py_1()
                .text_color(rgb(0xffffff))
                .child("Delete")
                .on_mouse_up(MouseButton::Left, on_delete),
        )
}
