use gpui::{App, MouseButton, MouseUpEvent, Window, div, prelude::*, rgb};

use super::filter::Filter;

pub fn filter_bar<H>(
    selected: Filter,
    mut on_select: impl FnMut(Filter) -> H,
) -> impl IntoElement
where
    H: Fn(&MouseUpEvent, &mut Window, &mut App) + 'static,
{
    div().flex().flex_row().gap_2().children(Filter::all().map(|filter| {
        let selected = selected == filter;
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
            .on_mouse_up(MouseButton::Left, on_select(filter))
    }))
}
