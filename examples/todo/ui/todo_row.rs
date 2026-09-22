use gpui::{Context, EventEmitter, MouseButton, div, prelude::*, rgb};
use crate::domain::Todo;

pub enum TodoRowEvent {
    Toggle,
    Delete,
}

pub struct TodoRow {
    pub todo: Todo,
}

impl TodoRow {
    pub fn new(todo: Todo) -> Self {
        Self { todo }
    }
}

impl EventEmitter<TodoRowEvent> for TodoRow {}

impl Render for TodoRow {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let completed = self.todo.completed;
        let text = self.todo.text.clone();

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
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|_, _, _, cx| cx.emit(TodoRowEvent::Toggle)),
                    ),
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
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|_, _, _, cx| cx.emit(TodoRowEvent::Delete)),
                    ),
            )
    }
}
