use std::time::Duration;

use homebase::rusqlite::{self, Connection, Transaction, params};
use homebase::{Error, Mutator, Query, Store};
use tempfile::tempdir;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS todos (
    id TEXT PRIMARY KEY NOT NULL,
    text TEXT NOT NULL,
    completed INTEGER NOT NULL DEFAULT 0
);
";

#[derive(Clone, Debug, PartialEq, Eq)]
struct Todo {
    id: String,
    text: String,
    completed: bool,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum Event {
    Created { id: String, text: String },
    Completed { id: String },
    Uncompleted { id: String },
    Deleted { id: String },
}

struct TodoMutator;

impl Mutator<Event> for TodoMutator {
    fn apply(&self, tx: &Transaction<'_>, event: &Event) -> rusqlite::Result<()> {
        match event {
            Event::Created { id, text } => {
                tx.execute(
                    "INSERT INTO todos (id, text, completed) VALUES (?1, ?2, 0)",
                    params![id, text],
                )?;
            }
            Event::Completed { id } => {
                tx.execute("UPDATE todos SET completed = 1 WHERE id = ?1", params![id])?;
            }
            Event::Uncompleted { id } => {
                tx.execute("UPDATE todos SET completed = 0 WHERE id = ?1", params![id])?;
            }
            Event::Deleted { id } => {
                tx.execute("DELETE FROM todos WHERE id = ?1", params![id])?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum TodoQuery {
    Active,
    Completed,
}

impl Query for TodoQuery {
    type Row = Todo;

    fn execute(&self, conn: &Connection) -> rusqlite::Result<Vec<Todo>> {
        let sql = match self {
            TodoQuery::Active => {
                "SELECT id, text, completed FROM todos WHERE completed = 0 ORDER BY id"
            }
            TodoQuery::Completed => {
                "SELECT id, text, completed FROM todos WHERE completed = 1 ORDER BY id"
            }
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| {
            Ok(Todo {
                id: row.get(0)?,
                text: row.get(1)?,
                completed: row.get::<_, i64>(2)? != 0,
            })
        })?;
        rows.collect()
    }
}

fn open() -> Store<Event> {
    Store::open(":memory:", SCHEMA, TodoMutator).unwrap()
}

fn created(id: &str, text: &str) -> Event {
    Event::Created {
        id: id.to_string(),
        text: text.to_string(),
    }
}

fn milk(completed: bool) -> Todo {
    Todo {
        id: "1".into(),
        text: "milk".into(),
        completed,
    }
}

fn wait_for(rx: &std::sync::mpsc::Receiver<Vec<Todo>>) -> Vec<Todo> {
    rx.recv_timeout(Duration::from_secs(1))
        .expect("timed out waiting for live query callback")
}

fn assert_no_callback(rx: &std::sync::mpsc::Receiver<Vec<Todo>>, msg: &str) {
    assert!(rx.recv_timeout(Duration::from_millis(50)).is_err(), "{msg}");
}

#[test]
fn create_notifies_active_query() {
    // Arrange
    let mut store = open();
    let (tx, rx) = std::sync::mpsc::channel();
    let _sub = store
        .subscribe(TodoQuery::Active, move |rows| {
            tx.send(rows.to_vec()).unwrap();
        })
        .unwrap();

    // Act
    store.commit(created("1", "milk"), &mut ()).unwrap();

    // Assert
    assert_eq!(wait_for(&rx), vec![milk(false)]);
}

#[test]
fn complete_moves_todo_between_queries() {
    // Arrange
    let mut store = open();
    let (active_tx, active_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let _active = store
        .subscribe(TodoQuery::Active, move |rows| {
            active_tx.send(rows.to_vec()).unwrap();
        })
        .unwrap();
    let _done = store
        .subscribe(TodoQuery::Completed, move |rows| {
            done_tx.send(rows.to_vec()).unwrap();
        })
        .unwrap();
    store.commit(created("1", "milk"), &mut ()).unwrap();
    assert_eq!(wait_for(&active_rx), vec![milk(false)]);
    assert_no_callback(&done_rx, "create should not notify completed query");

    // Act
    store.commit(Event::Completed { id: "1".into() }, &mut ()).unwrap();

    // Assert
    assert!(wait_for(&active_rx).is_empty());
    assert_eq!(wait_for(&done_rx), vec![milk(true)]);
}

#[test]
fn delete_removes_from_both_queries() {
    // Arrange
    let mut store = open();
    let (active_tx, active_rx) = std::sync::mpsc::channel();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let _active = store
        .subscribe(TodoQuery::Active, move |rows| {
            active_tx.send(rows.to_vec()).unwrap();
        })
        .unwrap();
    let _done = store
        .subscribe(TodoQuery::Completed, move |rows| {
            done_tx.send(rows.to_vec()).unwrap();
        })
        .unwrap();
    store.commit(created("1", "milk"), &mut ()).unwrap();
    wait_for(&active_rx);
    store.commit(Event::Completed { id: "1".into() }, &mut ()).unwrap();
    wait_for(&done_rx);
    wait_for(&active_rx);

    // Act
    store.commit(Event::Deleted { id: "1".into() }, &mut ()).unwrap();

    // Assert
    assert!(wait_for(&done_rx).is_empty());
    assert_no_callback(
        &active_rx,
        "active query was already empty; delete must not notify it",
    );
}

#[test]
fn complete_already_completed_does_not_notify() {
    // Arrange
    let mut store = open();
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let _done = store
        .subscribe(TodoQuery::Completed, move |rows| {
            done_tx.send(rows.to_vec()).unwrap();
        })
        .unwrap();
    store.commit(created("1", "milk"), &mut ()).unwrap();
    store.commit(Event::Completed { id: "1".into() }, &mut ()).unwrap();
    wait_for(&done_rx);

    // Act
    store.commit(Event::Completed { id: "1".into() }, &mut ()).unwrap();

    // Assert
    assert_no_callback(&done_rx, "equal result must not notify subscribers");
}

#[test]
fn reopen_same_file_keeps_todos() {
    // Arrange
    let dir = tempdir().unwrap();
    let path = dir.path().join("app.db");
    {
        let mut store = Store::open(&path, SCHEMA, TodoMutator).unwrap();
        store.commit(created("1", "milk"), &mut ()).unwrap();
    }

    // Act
    let mut store = Store::open(&path, SCHEMA, TodoMutator).unwrap();

    // Assert
    let rows = store.query(TodoQuery::Active).unwrap();
    assert_eq!(rows, vec![milk(false)]);
}

#[test]
fn query_after_create_returns_row() {
    // Arrange
    let mut store = open();

    // Act
    store.commit(created("1", "milk"), &mut ()).unwrap();

    // Assert
    let rows = store.query(TodoQuery::Active).unwrap();
    assert_eq!(rows, vec![milk(false)]);
}

#[test]
fn commit_updates_watched_query_without_requery() {
    // Arrange
    let mut store = open();
    let active = store.watch(TodoQuery::Active).unwrap();
    assert!(active.rows().is_empty());

    // Act
    store.commit(created("1", "milk"), &mut ()).unwrap();

    // Assert
    assert_eq!(active.rows(), vec![milk(false)]);
}

#[test]
fn commit_updates_every_watched_query() {
    // Arrange
    let mut store = open();
    let active = store.watch(TodoQuery::Active).unwrap();
    let completed = store.watch(TodoQuery::Completed).unwrap();
    store.commit(created("1", "milk"), &mut ()).unwrap();
    assert_eq!(active.rows(), vec![milk(false)]);
    assert!(completed.rows().is_empty());

    // Act
    store.commit(Event::Completed { id: "1".into() }, &mut ()).unwrap();

    // Assert
    assert!(active.rows().is_empty());
    assert_eq!(completed.rows(), vec![milk(true)]);
}

#[test]
fn many_commits_keep_watch_current() {
    // Arrange
    let mut store = open();
    let active = store.watch(TodoQuery::Active).unwrap();

    // Act
    store.commit(created("1", "milk"), &mut ()).unwrap();
    store.commit(created("2", "bread"), &mut ()).unwrap();
    store.commit(Event::Completed { id: "1".into() }, &mut ()).unwrap();

    // Assert
    assert_eq!(
        active.rows(),
        vec![Todo {
            id: "2".into(),
            text: "bread".into(),
            completed: false,
        }]
    );
}

#[test]
fn open_invalid_schema_is_sqlite_error() {
    // Arrange
    let schema = "not valid sql";

    // Act
    let err = match Store::<Event>::open(":memory:", schema, TodoMutator) {
        Err(err) => err,
        Ok(_) => panic!("expected sqlite error"),
    };

    // Assert
    assert!(matches!(err, Error::Sqlite(_)));
}

#[test]
fn dropped_subscribe_does_not_notify() {
    // Arrange
    let mut store = open();
    let (tx, rx) = std::sync::mpsc::channel();
    let sub = store
        .subscribe(TodoQuery::Active, move |rows| {
            tx.send(rows.to_vec()).unwrap();
        })
        .unwrap();

    // Act
    drop(sub);
    store.commit(created("1", "milk"), &mut ()).unwrap();

    // Assert
    assert_no_callback(&rx, "dropped subscribe must not notify");
}

#[test]
fn dropped_watch_does_not_rerun_query() {
    // Arrange
    let mut store = open();
    let fail = Rc::new(Cell::new(false));
    let live = store
        .watch(FlakyQuery {
            fail: fail.clone(),
        })
        .unwrap();
    drop(live);
    fail.set(true);

    // Act
    store.commit(created("1", "milk"), &mut ()).unwrap();

    // Assert — commit succeeds because the flaky query is no longer watched
    let rows = store.query(TodoQuery::Active).unwrap();
    assert_eq!(rows, vec![milk(false)]);
}

#[test]
fn dropping_one_watch_keeps_the_other_current() {
    // Arrange
    let mut store = open();
    let first = store.watch(TodoQuery::Active).unwrap();
    let second = store.watch(TodoQuery::Active).unwrap();
    drop(first);

    // Act
    store.commit(created("1", "milk"), &mut ()).unwrap();

    // Assert
    assert_eq!(second.rows(), vec![milk(false)]);
}
