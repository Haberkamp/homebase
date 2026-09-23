use gpui::{div, prelude::*};

pub fn todo_list(rows: impl IntoIterator<Item = impl IntoElement>) -> impl IntoElement {
    div()
        .id("todo-list")
        .flex()
        .flex_col()
        .gap_2()
        .flex_1()
        .w_full()
        .overflow_y_scroll()
        .children(rows)
}
