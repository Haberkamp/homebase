use homebase::rusqlite::{self, Row, Transaction};
use homebase::{Bind, Error, Mutator, Store, Table};

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

impl Table for Todo {
    const TABLE: &'static str = "todos";
    const COLUMNS: &'static [&'static str] = &["id", "text", "completed"];

    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            text: row.get(1)?,
            completed: row.get::<_, i64>(2)? != 0,
        })
    }

    fn values(&self) -> Vec<Bind> {
        vec![
            self.id.as_str().into(),
            self.text.as_str().into(),
            self.completed.into(),
        ]
    }
}

fn todo(id: &str, text: &str, completed: bool) -> Todo {
    Todo {
        id: id.into(),
        text: text.into(),
        completed,
    }
}

enum Event {
    Create(Todo),
    Complete { id: String },
    Rename { id: String, text: String },
    CompleteActive { id: String },
    Delete { id: String },
    DeleteCompleted,
    UpdateUnfiltered,
    DeleteUnfiltered,
    RawComplete { id: String },
}

struct TodoMutator;

impl Mutator<Event> for TodoMutator {
    fn apply(&self, tx: &Transaction<'_>, event: &Event) -> rusqlite::Result<()> {
        match event {
            Event::Create(row) => {
                Todo::create(tx, row)?;
            }
            Event::Complete { id } => {
                Todo::where_eq("id", id.as_str())
                    .set("completed", true)
                    .update(tx)?;
            }
            Event::Rename { id, text } => {
                Todo::where_eq("id", id.as_str())
                    .set("text", text.as_str())
                    .set("completed", false)
                    .update(tx)?;
            }
            Event::CompleteActive { id } => {
                Todo::where_eq("id", id.as_str())
                    .where_eq("completed", false)
                    .set("completed", true)
                    .update(tx)?;
            }
            Event::Delete { id } => {
                Todo::where_eq("id", id.as_str()).delete(tx)?;
            }
            Event::DeleteCompleted => {
                Todo::where_eq("completed", true).delete(tx)?;
            }
            Event::UpdateUnfiltered => {
                Todo::query().set("completed", true).update(tx)?;
            }
            Event::DeleteUnfiltered => {
                Todo::query().delete(tx)?;
            }
            Event::RawComplete { id } => {
                Todo::exec(
                    tx,
                    "UPDATE todos SET completed = 1 WHERE id = ?1",
                    [id.as_str()],
                )?;
            }
        }
        Ok(())
    }
}

fn open() -> Store<Event> {
    Store::open(":memory:", SCHEMA, TodoMutator).unwrap()
}

fn seed(store: &mut Store<Event>) {
    store
        .commit(Event::Create(todo("1", "milk", false)), &mut ())
        .unwrap();
    store
        .commit(Event::Create(todo("2", "bread", true)), &mut ())
        .unwrap();
    store
        .commit(Event::Create(todo("3", "eggs", false)), &mut ())
        .unwrap();
}

fn rows(store: &mut Store<Event>) -> Vec<Todo> {
    store.watch(Todo::query().order_by("id")).unwrap().rows()
}

fn expect_sqlite<T>(result: homebase::Result<T>) -> rusqlite::Error {
    match result {
        Err(Error::Sqlite(err)) => err,
        Ok(_) => panic!("expected sqlite error"),
    }
}

#[test]
fn create_inserts_row() {
    // Arrange
    let mut store = open();

    // Act
    store
        .commit(Event::Create(todo("1", "milk", false)), &mut ())
        .unwrap();

    // Assert
    assert_eq!(rows(&mut store), vec![todo("1", "milk", false)]);
}

#[test]
fn create_keeps_watch_current() {
    // Arrange
    let mut store = open();
    let live = store.watch(Todo::query().order_by("id")).unwrap();

    // Act
    store
        .commit(Event::Create(todo("1", "milk", false)), &mut ())
        .unwrap();

    // Assert
    assert_eq!(live.rows(), vec![todo("1", "milk", false)]);
}

#[test]
fn update_changes_matching_row() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    store
        .commit(Event::Complete { id: "1".into() }, &mut ())
        .unwrap();

    // Assert
    assert_eq!(
        rows(&mut store),
        vec![
            todo("1", "milk", true),
            todo("2", "bread", true),
            todo("3", "eggs", false),
        ]
    );
}

#[test]
fn update_leaves_other_rows() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    store
        .commit(Event::Complete { id: "3".into() }, &mut ())
        .unwrap();

    // Assert
    assert_eq!(
        rows(&mut store),
        vec![
            todo("1", "milk", false),
            todo("2", "bread", true),
            todo("3", "eggs", true),
        ]
    );
}

#[test]
fn update_sets_several_columns() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    store
        .commit(
            Event::Rename {
                id: "2".into(),
                text: "toast".into(),
            },
            &mut (),
        )
        .unwrap();

    // Assert
    assert_eq!(
        rows(&mut store),
        vec![
            todo("1", "milk", false),
            todo("2", "toast", false),
            todo("3", "eggs", false),
        ]
    );
}

#[test]
fn update_applies_extra_where() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    store
        .commit(Event::CompleteActive { id: "2".into() }, &mut ())
        .unwrap();

    // Assert
    assert_eq!(
        rows(&mut store),
        vec![
            todo("1", "milk", false),
            todo("2", "bread", true),
            todo("3", "eggs", false),
        ]
    );
}

#[test]
fn update_keeps_watch_current() {
    // Arrange
    let mut store = open();
    seed(&mut store);
    let live = store
        .watch(Todo::query().where_eq("completed", true).order_by("id"))
        .unwrap();

    // Act
    store
        .commit(Event::Complete { id: "1".into() }, &mut ())
        .unwrap();

    // Assert
    assert_eq!(
        live.rows(),
        vec![todo("1", "milk", true), todo("2", "bread", true)]
    );
}

#[test]
fn update_without_where_is_error() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let err = expect_sqlite(store.commit(Event::UpdateUnfiltered, &mut ()));

    // Assert
    assert!(err.to_string().contains("UPDATE requires WHERE"));
}

#[test]
fn delete_removes_matching_row() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    store
        .commit(Event::Delete { id: "2".into() }, &mut ())
        .unwrap();

    // Assert
    assert_eq!(
        rows(&mut store),
        vec![todo("1", "milk", false), todo("3", "eggs", false)]
    );
}

#[test]
fn delete_matching_set() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    store.commit(Event::DeleteCompleted, &mut ()).unwrap();

    // Assert
    assert_eq!(
        rows(&mut store),
        vec![todo("1", "milk", false), todo("3", "eggs", false)]
    );
}

#[test]
fn delete_keeps_watch_current() {
    // Arrange
    let mut store = open();
    seed(&mut store);
    let live = store.watch(Todo::query().order_by("id")).unwrap();

    // Act
    store
        .commit(Event::Delete { id: "1".into() }, &mut ())
        .unwrap();

    // Assert
    assert_eq!(
        live.rows(),
        vec![todo("2", "bread", true), todo("3", "eggs", false)]
    );
}

#[test]
fn delete_without_where_is_error() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let err = expect_sqlite(store.commit(Event::DeleteUnfiltered, &mut ()));

    // Assert
    assert!(err.to_string().contains("DELETE requires WHERE"));
}

#[test]
fn exec_runs_raw_sqlite() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    store
        .commit(Event::RawComplete { id: "1".into() }, &mut ())
        .unwrap();

    // Assert
    assert_eq!(
        rows(&mut store),
        vec![
            todo("1", "milk", true),
            todo("2", "bread", true),
            todo("3", "eggs", false),
        ]
    );
}

#[test]
fn exec_keeps_watch_current() {
    // Arrange
    let mut store = open();
    seed(&mut store);
    let live = store.watch(Todo::query().order_by("id")).unwrap();

    // Act
    store
        .commit(Event::RawComplete { id: "3".into() }, &mut ())
        .unwrap();

    // Assert
    assert_eq!(
        live.rows(),
        vec![
            todo("1", "milk", false),
            todo("2", "bread", true),
            todo("3", "eggs", true),
        ]
    );
}
