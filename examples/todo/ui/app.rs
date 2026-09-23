use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{App, Context, Entity, FocusHandle, Focusable, Window, div, prelude::*, rgb};
use homestead::{Live, Select, Store, Table};

use crate::domain::{Event, Todo, TodoMutator};

use super::filter::Filter;
use super::filter_bar::filter_bar;
use super::header::{db_path, header, migrations_path};
use super::todo_input::TodoInput;
use super::todo_list::todo_list;
use super::todo_row::todo_row;

pub struct TodoApp {
    store: Store<Event>,
    active: Live<Select<Todo>>,
    completed: Live<Select<Todo>>,
    all: Live<Select<Todo>>,
    filter: Filter,
    input: Entity<TodoInput>,
}

impl TodoApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let path = db_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut store =
            Store::open(&path, migrations_path(), TodoMutator).expect("open sqlite database");
        let active = store
            .watch(Todo::query().where_eq("completed", false).order_by("id"))
            .expect("watch active todos");
        let completed = store
            .watch(Todo::query().where_eq("completed", true).order_by("id"))
            .expect("watch completed todos");
        let all = store
            .watch(Todo::query().order_by("id"))
            .expect("watch all todos");

        let app = cx.weak_entity();
        let input = cx.new(|cx| {
            TodoInput::new(window, cx, move |text, cx| {
                if let Some(app) = app.upgrade() {
                    app.update(cx, |this, cx| this.add_todo(text, cx));
                }
            })
        });

        Self {
            store,
            active,
            completed,
            all,
            filter: Filter::All,
            input,
        }
    }

    fn watched_rows(&self) -> Vec<Todo> {
        match self.filter {
            Filter::All => self.all.rows(),
            Filter::Active => self.active.rows(),
            Filter::Completed => self.completed.rows(),
        }
    }

    fn set_filter(&mut self, filter: Filter, cx: &mut Context<Self>) {
        if self.filter == filter {
            return;
        }
        self.filter = filter;
        cx.notify();
    }

    fn add_todo(&mut self, text: String, cx: &mut Context<Self>) {
        self.store
            .commit(Event::Created { id: new_id(), text })
            .expect("commit created");
        cx.notify();
    }

    fn toggle(&mut self, id: String, completed: bool, cx: &mut Context<Self>) {
        let event = if completed {
            Event::Uncompleted { id }
        } else {
            Event::Completed { id }
        };
        self.store.commit(event).expect("commit toggle");
        cx.notify();
    }

    fn delete(&mut self, id: String, cx: &mut Context<Self>) {
        self.store
            .commit(Event::Deleted { id })
            .expect("commit deleted");
        cx.notify();
    }
}

impl Focusable for TodoApp {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl Render for TodoApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = self.watched_rows().into_iter().map(|todo| {
            let id = todo.id.clone();
            let completed = todo.completed;
            let delete_id = todo.id.clone();
            todo_row(
                todo,
                cx.listener(move |this, _, _, cx| this.toggle(id.clone(), completed, cx)),
                cx.listener(move |this, _, _, cx| this.delete(delete_id.clone(), cx)),
            )
        });

        let filter = self.filter;

        div()
            .flex()
            .flex_col()
            .gap_4()
            .bg(rgb(0x1a1a1a))
            .size_full()
            .p_6()
            .child(header())
            .child(self.input.clone())
            .child(filter_bar(filter, |next| {
                cx.listener(move |this, _, _, cx| this.set_filter(next, cx))
            }))
            .child(todo_list(rows))
    }
}

fn new_id() -> String {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("todo-{ns}")
}
