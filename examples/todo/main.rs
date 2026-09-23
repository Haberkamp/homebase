//! Basic GPUI todo app backed by the homestead SQLite store.
//!
//! ```sh
//! cargo run --example todo
//! ```
//!
//! UI lives in `ui/`. The database file is `examples/todo/todos.db`.

mod domain;
mod ui;

use gpui::{
    App, Application, Bounds, Focusable, KeyBinding, WindowBounds, WindowOptions, prelude::*, px,
    size,
};

use ui::{AddTodo, Backspace, ClearInput, TodoApp};

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(520.), px(640.)), cx);
        cx.bind_keys([
            KeyBinding::new("enter", AddTodo, None),
            KeyBinding::new("backspace", Backspace, None),
            KeyBinding::new("escape", ClearInput, None),
        ]);

        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| TodoApp::new(window, cx)),
            )
            .unwrap();

        window
            .update(cx, |view, window, cx| {
                window.focus(&view.focus_handle(cx));
                cx.activate(true);
            })
            .unwrap();
    });
}
