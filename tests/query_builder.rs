use homebase::rusqlite::{self, Row, Transaction, params};
use homebase::{Mutator, Select, Store, Table};

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

    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            text: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            completed: row.get::<_, i64>(2)? != 0,
        })
    }
}

enum Event {
    Created { id: String, text: String },
    CreatedNullText { id: String },
    Completed { id: String },
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
            Event::CreatedNullText { id } => {
                tx.execute(
                    "INSERT INTO todos (id, text, completed) VALUES (?1, NULL, 0)",
                    params![id],
                )?;
            }
            Event::Completed { id } => {
                tx.execute("UPDATE todos SET completed = 1 WHERE id = ?1", params![id])?;
            }
        }
        Ok(())
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

fn todo(id: &str, text: &str, completed: bool) -> Todo {
    Todo {
        id: id.into(),
        text: text.into(),
        completed,
    }
}

fn seed(store: &mut Store<Event>) {
    store.commit(created("1", "milk"), &mut ()).unwrap();
    store.commit(created("2", "bread"), &mut ()).unwrap();
    store.commit(created("3", "eggs"), &mut ()).unwrap();
    store
        .commit(Event::Completed { id: "2".into() }, &mut ())
        .unwrap();
}

#[test]
fn where_eq_filters_column() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let active = store
        .watch(Todo::query().where_eq("completed", false).order_by("id"))
        .unwrap();

    // Assert
    assert_eq!(
        active.rows(),
        vec![todo("1", "milk", false), todo("3", "eggs", false)]
    );
}

#[test]
fn where_op_compares_with_operator() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let later = store
        .watch(Todo::query().where_op("id", ">", "1").order_by("id"))
        .unwrap();

    // Assert
    assert_eq!(
        later.rows(),
        vec![todo("2", "bread", true), todo("3", "eggs", false)]
    );
}

#[test]
fn or_where_eq_widens_filter() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = store
        .watch(
            Todo::query()
                .where_eq("id", "1")
                .or_where_eq("id", "3")
                .order_by("id"),
        )
        .unwrap();

    // Assert
    assert_eq!(
        rows.rows(),
        vec![todo("1", "milk", false), todo("3", "eggs", false)]
    );
}

#[test]
fn where_in_matches_any_value() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = store
        .watch(
            Todo::query()
                .where_in("id", ["1", "2"])
                .order_by("id"),
        )
        .unwrap();

    // Assert
    assert_eq!(
        rows.rows(),
        vec![todo("1", "milk", false), todo("2", "bread", true)]
    );
}

#[test]
fn where_null_and_not_null() {
    // Arrange
    let mut store = Store::open(
        ":memory:",
        "
        CREATE TABLE todos (
            id TEXT PRIMARY KEY NOT NULL,
            text TEXT,
            completed INTEGER NOT NULL DEFAULT 0
        );
        ",
        TodoMutator,
    )
    .unwrap();
    store.commit(created("1", "milk"), &mut ()).unwrap();
    store
        .commit(
            Event::CreatedNullText { id: "2".into() },
            &mut (),
        )
        .unwrap();

    // Act
    let present = store
        .watch(Todo::query().where_not_null("text").order_by("id"))
        .unwrap();
    let missing = store
        .watch(Todo::query().where_null("text").order_by("id"))
        .unwrap();

    // Assert
    assert_eq!(present.rows(), vec![todo("1", "milk", false)]);
    assert_eq!(
        missing.rows(),
        vec![Todo {
            id: "2".into(),
            text: String::new(),
            completed: false,
        }]
    );
}

#[test]
fn order_by_desc_and_limit() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = store
        .watch(Todo::query().order_by_desc("id").limit(2))
        .unwrap();

    // Assert
    assert_eq!(
        rows.rows(),
        vec![todo("3", "eggs", false), todo("2", "bread", true)]
    );
}

#[test]
fn latest_and_offset() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = store
        .watch(Todo::query().latest("id").offset(1).limit(1))
        .unwrap();

    // Assert
    assert_eq!(rows.rows(), vec![todo("2", "bread", true)]);
}

#[test]
fn where_raw_escape_hatch() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = store
        .watch(
            Todo::query()
                .where_raw("completed = 0 AND text != ?", ["milk"])
                .order_by("id"),
        )
        .unwrap();

    // Assert
    assert_eq!(rows.rows(), vec![todo("3", "eggs", false)]);
}

#[test]
fn sql_escape_hatch_runs_custom_sqlite() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = store
        .watch(Select::<Todo>::sql(
            "SELECT id, text, completed FROM todos WHERE text LIKE ?1 ORDER BY id",
            ["%e%"],
        ))
        .unwrap();

    // Assert
    assert_eq!(
        rows.rows(),
        vec![todo("2", "bread", true), todo("3", "eggs", false)]
    );
}

#[test]
fn commit_keeps_select_watch_current() {
    // Arrange
    let mut store = open();
    store.commit(created("1", "milk"), &mut ()).unwrap();

    // Act
    let active = store
        .watch(Todo::query().where_eq("completed", false).order_by("id"))
        .unwrap();
    store
        .commit(Event::Completed { id: "1".into() }, &mut ())
        .unwrap();

    // Assert
    assert!(active.rows().is_empty());
}
