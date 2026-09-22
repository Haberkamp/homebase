use gpui::{Context, EventEmitter, MouseButton, div, prelude::*, rgb};

use super::filter::Filter;

pub enum FilterBarEvent {
    Changed,
}

pub struct FilterBar {
    selected: Filter,
}

impl FilterBar {
    pub fn new() -> Self {
        Self {
            selected: Filter::All,
        }
    }

    pub fn selected(&self) -> Filter {
        self.selected
    }

    fn select(&mut self, filter: Filter, cx: &mut Context<Self>) {
        if self.selected == filter {
            return;
        }
        self.selected = filter;
        cx.emit(FilterBarEvent::Changed);
        cx.notify();
    }
}

impl EventEmitter<FilterBarEvent> for FilterBar {}

impl Render for FilterBar {
    fn render(&mut self, _window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tabs = Vec::new();
        for filter in Filter::all() {
            let selected = self.selected == filter;
            tabs.push(
                div()
                    .px_3()
                    .py_1()
                    .rounded_lg()
                    .cursor_pointer()
                    .bg(if selected {
                        rgb(0x4a9eff)
                    } else {
                        rgb(0x2a2a2a)
                    })
                    .text_color(rgb(0xe0e0e0))
                    .child(filter.label())
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| this.select(filter, cx)),
                    ),
            );
        }
        div().flex().flex_row().gap_2().children(tabs)
    }
}
