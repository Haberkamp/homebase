use std::cell::Cell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

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
    type Row = Vec<Todo>;

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

fn migrate_dir(sql: &str) -> tempfile::TempDir {
    let dir = tempdir().unwrap();
    if !sql.trim().is_empty() {
        std::fs::write(dir.path().join("001.sql"), sql).unwrap();
    }
    dir
}

fn open() -> Store<Event> {
    let dir = migrate_dir(SCHEMA);
    Store::open(":memory:", dir.path(), TodoMutator).unwrap()
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

fn expect_sqlite<T>(result: homebase::Result<T>) -> rusqlite::Error {
    match result {
        Err(Error::Sqlite(err)) => err,
        Err(Error::Io(err)) => panic!("expected sqlite error, got io: {err}"),
        Ok(_) => panic!("expected sqlite error"),
    }
}

#[derive(Clone)]
struct FlakyQuery {
    fail: Rc<Cell<bool>>,
}

impl PartialEq for FlakyQuery {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for FlakyQuery {}

impl Hash for FlakyQuery {
    fn hash<H: Hasher>(&self, state: &mut H) {
        0u8.hash(state);
    }
}

impl Query for FlakyQuery {
    type Row = Vec<Todo>;

    fn execute(&self, conn: &Connection) -> rusqlite::Result<Vec<Todo>> {
        if self.fail.get() {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        TodoQuery::Active.execute(conn)
    }
}

#[test]
fn create_updates_active_watch() {
    // Arrange
    let mut store = open();
    let active = store.watch(TodoQuery::Active).unwrap();

    // Act
    store.commit(created("1", "milk"), &mut ()).unwrap();

    // Assert
    assert_eq!(active.rows(), vec![milk(false)]);
}

#[test]
fn complete_moves_todo_between_watches() {
    // Arrange
    let mut store = open();
    let active = store.watch(TodoQuery::Active).unwrap();
    let completed = store.watch(TodoQuery::Completed).unwrap();
    store.commit(created("1", "milk"), &mut ()).unwrap();
    assert_eq!(active.rows(), vec![milk(false)]);
    assert!(completed.rows().is_empty());

    // Act
    store
        .commit(Event::Completed { id: "1".into() }, &mut ())
        .unwrap();

    // Assert
    assert!(active.rows().is_empty());
    assert_eq!(completed.rows(), vec![milk(true)]);
}

#[test]
fn delete_clears_completed_watch() {
    // Arrange
    let mut store = open();
    let active = store.watch(TodoQuery::Active).unwrap();
    let completed = store.watch(TodoQuery::Completed).unwrap();
    store.commit(created("1", "milk"), &mut ()).unwrap();
    store
        .commit(Event::Completed { id: "1".into() }, &mut ())
        .unwrap();
    assert!(active.rows().is_empty());
    assert_eq!(completed.rows(), vec![milk(true)]);

    // Act
    store
        .commit(Event::Deleted { id: "1".into() }, &mut ())
        .unwrap();

    // Assert
    assert!(active.rows().is_empty());
    assert!(completed.rows().is_empty());
}

#[test]
fn complete_already_completed_keeps_same_rows() {
    // Arrange
    let mut store = open();
    let completed = store.watch(TodoQuery::Completed).unwrap();
    store.commit(created("1", "milk"), &mut ()).unwrap();
    store
        .commit(Event::Completed { id: "1".into() }, &mut ())
        .unwrap();
    assert_eq!(completed.rows(), vec![milk(true)]);

    // Act
    store
        .commit(Event::Completed { id: "1".into() }, &mut ())
        .unwrap();

    // Assert
    assert_eq!(completed.rows(), vec![milk(true)]);
}

#[test]
fn reopen_same_file_keeps_todos() {
    // Arrange
    let dir = tempdir().unwrap();
    let path = dir.path().join("app.db");
    let migrations = migrate_dir(SCHEMA);
    {
        let mut store = Store::open(&path, migrations.path(), TodoMutator).unwrap();
        store.commit(created("1", "milk"), &mut ()).unwrap();
    }

    // Act
    let mut store = Store::open(&path, migrations.path(), TodoMutator).unwrap();
    let active = store.watch(TodoQuery::Active).unwrap();

    // Assert
    assert_eq!(active.rows(), vec![milk(false)]);
}

#[test]
fn many_commits_keep_watch_current() {
    // Arrange
    let mut store = open();
    let active = store.watch(TodoQuery::Active).unwrap();

    // Act
    store.commit(created("1", "milk"), &mut ()).unwrap();
    store.commit(created("2", "bread"), &mut ()).unwrap();
    store
        .commit(Event::Completed { id: "1".into() }, &mut ())
        .unwrap();

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
    let migrations = migrate_dir("not valid sql");

    // Act
    let err = expect_sqlite(Store::<Event>::open(
        ":memory:",
        migrations.path(),
        TodoMutator,
    ));

    // Assert
    assert!(err.to_string().contains("syntax"));
}

#[test]
fn open_directory_path_is_sqlite_error() {
    // Arrange
    let dir = tempdir().unwrap();
    let migrations = migrate_dir(SCHEMA);

    // Act
    let err = expect_sqlite(Store::<Event>::open(
        dir.path(),
        migrations.path(),
        TodoMutator,
    ));

    // Assert
    assert!(!err.to_string().is_empty());
}

#[test]
fn duplicate_create_is_sqlite_error_and_rolls_back() {
    // Arrange
    let mut store = open();
    let active = store.watch(TodoQuery::Active).unwrap();
    store.commit(created("1", "milk"), &mut ()).unwrap();

    // Act
    let err = expect_sqlite(store.commit(created("1", "again"), &mut ()));

    // Assert
    assert!(err.to_string().contains("UNIQUE constraint failed"));
    assert_eq!(active.rows(), vec![milk(false)]);
}

#[test]
fn watch_missing_table_is_sqlite_error() {
    // Arrange
    let migrations = migrate_dir("");
    let mut store = Store::open(":memory:", migrations.path(), TodoMutator).unwrap();

    // Act
    let err = expect_sqlite(store.watch(TodoQuery::Active));

    // Assert
    assert!(err.to_string().contains("no such table"));
}

#[test]
fn commit_without_schema_is_sqlite_error() {
    // Arrange
    let migrations = migrate_dir("");
    let mut store = Store::open(":memory:", migrations.path(), TodoMutator).unwrap();

    // Act
    let err = expect_sqlite(store.commit(created("1", "milk"), &mut ()));

    // Assert
    assert!(err.to_string().contains("no such table"));
}

#[test]
fn refresh_query_failure_is_sqlite_error() {
    // Arrange
    let mut store = open();
    let fail = Rc::new(Cell::new(false));
    let _live = store.watch(FlakyQuery { fail: fail.clone() }).unwrap();
    fail.set(true);

    // Act
    let err = expect_sqlite(store.commit(created("1", "milk"), &mut ()));

    // Assert
    assert!(err.to_string().contains("Query returned no rows"));
}

#[test]
fn dropped_watch_does_not_rerun_query() {
    // Arrange
    let mut store = open();
    let fail = Rc::new(Cell::new(false));
    let live = store.watch(FlakyQuery { fail: fail.clone() }).unwrap();
    drop(live);
    fail.set(true);

    // Act
    store.commit(created("1", "milk"), &mut ()).unwrap();

    // Assert — commit succeeds because the flaky query is no longer watched
    let active = store.watch(TodoQuery::Active).unwrap();
    assert_eq!(active.rows(), vec![milk(false)]);
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
