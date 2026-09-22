use std::time::{SystemTime, UNIX_EPOCH};

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Subscription, Window, div, prelude::*, rgb,
};
use homebase::{Live, Store};

use crate::domain::{Event, SCHEMA, Todo, TodoMutator, TodoQuery};

use super::filter::Filter;
use super::filter_bar::{FilterBar, FilterBarEvent};
use super::header::{db_path, header};
use super::todo_input::{TodoInput, TodoInputEvent};
use super::todo_list::{TodoList, TodoListEvent};

pub struct TodoApp {
    store: Store<Event>,
    active: Live<TodoQuery>,
    completed: Live<TodoQuery>,
    all: Live<TodoQuery>,
    input: Entity<TodoInput>,
    filter_bar: Entity<FilterBar>,
    list: Entity<TodoList>,
    _subscriptions: Vec<Subscription>,
}

impl TodoApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let path = db_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut store = Store::open(&path, SCHEMA, TodoMutator).expect("open sqlite database");
        let active = store.watch(TodoQuery::Active).expect("watch active todos");
        let completed = store
            .watch(TodoQuery::Completed)
            .expect("watch completed todos");
        let all = store.watch(TodoQuery::All).expect("watch all todos");

        let input = cx.new(|cx| TodoInput::new(window, cx));
        let filter_bar = cx.new(|_| FilterBar::new());
        let list = cx.new(|_| TodoList::new());
        list.update(cx, |list, cx| list.set_todos(all.rows(), cx));

        let mut app = Self {
            store,
            active,
            completed,
            all,
            input: input.clone(),
            filter_bar: filter_bar.clone(),
            list: list.clone(),
            _subscriptions: Vec::new(),
        };

        app._subscriptions
            .push(cx.subscribe(&input, |this, _, event, cx| match event {
                TodoInputEvent::Submitted(text) => this.add_todo(text.clone(), cx),
            }));
        app._subscriptions
            .push(cx.subscribe(&filter_bar, |this, _, event, cx| {
                let FilterBarEvent::Changed = event;
                this.sync_list(cx);
            }));
        app._subscriptions
            .push(cx.subscribe(&list, |this, _, event, cx| match event {
                TodoListEvent::Toggle { id, completed } => this.toggle(id.clone(), *completed, cx),
                TodoListEvent::Delete { id } => this.delete(id.clone(), cx),
            }));

        app
    }

    fn filter(&self, cx: &App) -> Filter {
        self.filter_bar.read(cx).selected()
    }

    fn watched_rows(&self, cx: &App) -> Vec<Todo> {
        match self.filter(cx).query() {
            TodoQuery::All => self.all.rows(),
            TodoQuery::Active => self.active.rows(),
            TodoQuery::Completed => self.completed.rows(),
        }
    }

    fn sync_list(&mut self, cx: &mut Context<Self>) {
        let todos = self.watched_rows(cx);
        self.list.update(cx, |list, cx| list.set_todos(todos, cx));
    }

    fn add_todo(&mut self, text: String, cx: &mut Context<Self>) {
        self.store
            .commit(Event::Created { id: new_id(), text })
            .expect("commit created");
        self.sync_list(cx);
    }

    fn toggle(&mut self, id: String, completed: bool, cx: &mut Context<Self>) {
        let event = if completed {
            Event::Uncompleted { id }
        } else {
            Event::Completed { id }
        };
        self.store.commit(event).expect("commit toggle");
        self.sync_list(cx);
    }

    fn delete(&mut self, id: String, cx: &mut Context<Self>) {
        self.store
            .commit(Event::Deleted { id })
            .expect("commit deleted");
        self.sync_list(cx);
    }
}

impl Focusable for TodoApp {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl Render for TodoApp {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            .bg(rgb(0x1a1a1a))
            .size_full()
            .p_6()
            .child(header())
            .child(self.input.clone())
            .child(self.filter_bar.clone())
            .child(self.list.clone())
    }
}

fn new_id() -> String {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    format!("todo-{ns}")
}
