use gpui::{Context, Entity, EventEmitter, Subscription, div, prelude::*};
use crate::domain::Todo;

use super::todo_row::{TodoRow, TodoRowEvent};

pub enum TodoListEvent {
    Toggle { id: String, completed: bool },
    Delete { id: String },
}

pub struct TodoList {
    rows: Vec<Entity<TodoRow>>,
    _subscriptions: Vec<Subscription>,
}

impl TodoList {
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            _subscriptions: Vec::new(),
        }
    }

    pub fn set_todos(&mut self, todos: Vec<Todo>, cx: &mut Context<Self>) {
        self.rows.clear();
        self._subscriptions.clear();
        for todo in todos {
            let row = cx.new(|_| TodoRow::new(todo));
            let sub = cx.subscribe(&row, |_this, row, event, cx| {
                let todo = row.read(cx).todo.clone();
                match event {
                    TodoRowEvent::Toggle => cx.emit(TodoListEvent::Toggle {
                        id: todo.id,
                        completed: todo.completed,
                    }),
                    TodoRowEvent::Delete => cx.emit(TodoListEvent::Delete { id: todo.id }),
                }
            });
            self.rows.push(row);
            self._subscriptions.push(sub);
        }
        cx.notify();
    }
}

impl EventEmitter<TodoListEvent> for TodoList {}

impl Render for TodoList {
    fn render(&mut self, _window: &mut gpui::Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("todo-list")
            .flex()
            .flex_col()
            .gap_2()
            .flex_1()
            .w_full()
            .overflow_y_scroll()
            .children(self.rows.clone())
    }
}
