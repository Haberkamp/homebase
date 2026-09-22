use std::time::Duration;

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, KeyDownEvent, MouseButton, MouseDownEvent,
    MouseUpEvent, Subscription, Window, actions, div, prelude::*, px, rgb,
};

actions!(todo, [AddTodo, Backspace, ClearInput]);

pub enum TodoInputEvent {
    Submitted(String),
}

pub struct TodoInput {
    text: String,
    focus_handle: FocusHandle,
    caret_visible: bool,
    blink_epoch: u64,
    _subscriptions: Vec<Subscription>,
}

impl TodoInput {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let focus = cx.on_focus(&focus_handle, window, |this, _, cx| this.start_caret(cx));
        let blur = cx.on_blur(&focus_handle, window, |this, _, cx| this.stop_caret(cx));
        Self {
            text: String::new(),
            focus_handle,
            caret_visible: true,
            blink_epoch: 0,
            _subscriptions: vec![focus, blur],
        }
    }

    fn add_todo(&mut self, _: &AddTodo, _: &mut Window, cx: &mut Context<Self>) {
        self.submit(cx);
    }

    fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        if !self.text.is_empty() {
            self.text.pop();
            self.start_caret(cx);
        }
    }

    fn clear_input(&mut self, _: &ClearInput, _: &mut Window, cx: &mut Context<Self>) {
        self.text.clear();
        self.start_caret(cx);
    }

    fn on_add_click(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        self.submit(cx);
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        let text = self.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.text.clear();
        self.start_caret(cx);
        cx.emit(TodoInputEvent::Submitted(text));
    }

    fn start_caret(&mut self, cx: &mut Context<Self>) {
        self.caret_visible = true;
        self.blink_epoch = self.blink_epoch.wrapping_add(1);
        let epoch = self.blink_epoch;
        cx.notify();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(530))
                    .await;
                let keep_blinking = this
                    .update(cx, |this, cx| {
                        if this.blink_epoch != epoch {
                            return false;
                        }
                        this.caret_visible = !this.caret_visible;
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !keep_blinking {
                    break;
                }
            }
        })
        .detach();
    }

    fn stop_caret(&mut self, cx: &mut Context<Self>) {
        self.blink_epoch = self.blink_epoch.wrapping_add(1);
        self.caret_visible = false;
        cx.notify();
    }

    fn focus_field(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle);
        self.start_caret(cx);
    }
}

impl EventEmitter<TodoInputEvent> for TodoInput {}

impl Focusable for TodoInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for TodoInput {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focused = self.focus_handle.is_focused(window);
        let show_caret = focused && self.caret_visible;
        let empty = self.text.is_empty();
        let label = if empty && !focused {
            "Type a todo, then Enter".to_string()
        } else {
            self.text.clone()
        };

        div()
            .flex()
            .flex_row()
            .gap_3()
            .on_action(cx.listener(Self::add_todo))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::clear_input))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if let Some(key_char) = &event.keystroke.key_char {
                    if key_char.len() == 1 && !event.keystroke.modifiers.modified() {
                        this.text.push_str(key_char);
                        this.start_caret(cx);
                    }
                }
            }))
            .child(
                div()
                    .id("todo-input")
                    .track_focus(&self.focus_handle)
                    .flex()
                    .flex_1()
                    .flex_row()
                    .items_center()
                    .h(px(44.))
                    .bg(if focused {
                        rgb(0x243044)
                    } else {
                        rgb(0x2a2a2a)
                    })
                    .border_1()
                    .border_color(if focused {
                        rgb(0x4a9eff)
                    } else {
                        rgb(0x444444)
                    })
                    .rounded_lg()
                    .px_4()
                    .cursor_text()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseDownEvent, window, cx| {
                            this.focus_field(window, cx)
                        }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .text_color(if empty { rgb(0x888888) } else { rgb(0xe0e0e0) })
                            .child(label)
                            .when(show_caret, |el| {
                                el.child(div().w(px(2.)).h(px(18.)).ml_0p5().bg(rgb(0x4a9eff)))
                            }),
                    ),
            )
            .child(
                div()
                    .bg(rgb(0x4a9eff))
                    .hover(|style| style.bg(rgb(0x357abd)).cursor_pointer())
                    .rounded_lg()
                    .px_6()
                    .py_3()
                    .text_color(rgb(0xffffff))
                    .child("Add")
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_add_click)),
            )
    }
}
