use homebase::rusqlite::{self, Row, Transaction, params};
use homebase::{Error, Mutator, Select, Store, Table};
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

impl Table for Todo {
    const COLUMNS: &'static [&'static str] = &["id", "text", "completed"];

    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            text: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            completed: row.get::<_, i64>(2)? != 0,
        })
    }

    fn values(&self) -> Vec<homebase::Bind> {
        vec![
            self.id.as_str().into(),
            self.text.as_str().into(),
            self.completed.into(),
        ]
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

fn open_nullable() -> Store<Event> {
    let dir = migrate_dir(
        "
        CREATE TABLE todos (
            id TEXT PRIMARY KEY NOT NULL,
            text TEXT,
            completed INTEGER NOT NULL DEFAULT 0
        );
        ",
    );
    Store::open(":memory:", dir.path(), TodoMutator).unwrap()
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

fn milk() -> Todo {
    todo("1", "milk", false)
}
fn bread() -> Todo {
    todo("2", "bread", true)
}
fn eggs() -> Todo {
    todo("3", "eggs", false)
}

fn seed(store: &mut Store<Event>) {
    store.commit(created("1", "milk"), &mut ()).unwrap();
    store.commit(created("2", "bread"), &mut ()).unwrap();
    store.commit(created("3", "eggs"), &mut ()).unwrap();
    store
        .commit(Event::Completed { id: "2".into() }, &mut ())
        .unwrap();
}

fn seed_nullable(store: &mut Store<Event>) {
    store.commit(created("1", "milk"), &mut ()).unwrap();
    store
        .commit(Event::CreatedNullText { id: "2".into() }, &mut ())
        .unwrap();
}

fn null_text() -> Todo {
    Todo {
        id: "2".into(),
        text: String::new(),
        completed: false,
    }
}

fn run(store: &mut Store<Event>, query: Select<Todo>) -> Vec<Todo> {
    store.watch(query).unwrap().rows()
}

#[test]
fn query_returns_every_row() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().order_by("id"));

    // Assert
    assert_eq!(rows, vec![milk(), bread(), eggs()]);
}

#[test]
fn sql_runs_custom_sqlite() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Select::<Todo>::sql(
            "SELECT id, text, completed FROM todos WHERE text LIKE ?1 ORDER BY id",
            ["%e%"],
        ),
    );

    // Assert
    assert_eq!(rows, vec![bread(), eggs()]);
}

#[test]
fn select_keeps_column_order_for_from_row() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .select(["id", "text", "completed"])
            .where_eq("id", "1"),
    );

    // Assert
    assert_eq!(rows, vec![milk()]);
}

#[test]
fn distinct_does_not_collapse_rows_that_differ() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().distinct().order_by("id"));

    // Assert
    assert_eq!(rows, vec![milk(), bread(), eggs()]);
}

#[test]
fn where_eq_matches_column() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query().where_eq("completed", false).order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), eggs()]);
}

#[test]
fn where_ne_excludes_value() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().where_ne("id", "1").order_by("id"));

    // Assert
    assert_eq!(rows, vec![bread(), eggs()]);
}

#[test]
fn where_lt_is_strictly_less() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().where_lt("id", "2").order_by("id"));

    // Assert
    assert_eq!(rows, vec![milk()]);
}

#[test]
fn where_lte_includes_boundary() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().where_lte("id", "2").order_by("id"));

    // Assert
    assert_eq!(rows, vec![milk(), bread()]);
}

#[test]
fn where_gt_is_strictly_greater() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().where_gt("id", "2").order_by("id"));

    // Assert
    assert_eq!(rows, vec![eggs()]);
}

#[test]
fn where_gte_includes_boundary() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().where_gte("id", "2").order_by("id"));

    // Assert
    assert_eq!(rows, vec![bread(), eggs()]);
}

#[test]
fn or_where_eq_ors_with_previous_clause() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_eq("id", "1")
            .or_where_eq("id", "3")
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), eggs()]);
}

#[test]
fn or_where_ne_ors_inequality() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_eq("id", "1")
            .or_where_ne("id", "2")
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), eggs()]);
}

#[test]
fn where_op_uses_given_operator() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query().where_op("id", ">", "1").order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![bread(), eggs()]);
}

#[test]
fn or_where_op_ors_custom_operator() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_eq("id", "1")
            .or_where_op("id", ">", "2")
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), eggs()]);
}

#[test]
fn where_in_matches_any_listed_value() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query().where_in("id", ["1", "2"]).order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), bread()]);
}

#[test]
fn where_in_empty_matches_nothing() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query().where_in("id", Vec::<&str>::new()),
    );

    // Assert
    assert!(rows.is_empty());
}

#[test]
fn where_not_in_excludes_listed_values() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_not_in("id", ["1", "3"])
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![bread()]);
}

#[test]
fn or_where_in_ors_membership() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_eq("id", "1")
            .or_where_in("id", ["3"])
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), eggs()]);
}

#[test]
fn or_where_not_in_ors_exclusion() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_eq("id", "1")
            .or_where_not_in("id", ["1", "2"])
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), eggs()]);
}

#[test]
fn where_null_matches_null_column() {
    // Arrange
    let mut store = open_nullable();
    seed_nullable(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query().where_null("text").order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![null_text()]);
}

#[test]
fn where_not_null_matches_present_column() {
    // Arrange
    let mut store = open_nullable();
    seed_nullable(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query().where_not_null("text").order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk()]);
}

#[test]
fn or_where_null_ors_null_check() {
    // Arrange
    let mut store = open_nullable();
    seed_nullable(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_eq("id", "1")
            .or_where_null("text")
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), null_text()]);
}

#[test]
fn or_where_not_null_ors_present_check() {
    // Arrange
    let mut store = open_nullable();
    seed_nullable(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_null("text")
            .or_where_not_null("text")
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), null_text()]);
}

#[test]
fn where_between_is_inclusive() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query().where_between("id", "1", "2").order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), bread()]);
}

#[test]
fn where_not_between_excludes_range() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_not_between("id", "1", "2")
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![eggs()]);
}

#[test]
fn or_where_between_ors_range() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_eq("id", "3")
            .or_where_between("id", "1", "1")
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), eggs()]);
}

#[test]
fn where_like_matches_pattern() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query().where_like("text", "%e%").order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![bread(), eggs()]);
}

#[test]
fn where_not_like_excludes_pattern() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query().where_not_like("text", "%e%").order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk()]);
}

#[test]
fn or_where_like_ors_pattern() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_eq("id", "1")
            .or_where_like("text", "%gg%")
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), eggs()]);
}

#[test]
fn or_where_not_like_ors_negated_pattern() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_eq("id", "2")
            .or_where_not_like("text", "%e%")
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), bread()]);
}

#[test]
fn where_column_compares_two_columns() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_column("id", "!=", "text")
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), bread(), eggs()]);
}

#[test]
fn or_where_column_ors_column_compare() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_eq("id", "1")
            .or_where_column("id", "=", "text")
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk()]);
}

#[test]
fn where_raw_appends_sql_fragment() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_raw("completed = 0 AND text != ?", ["milk"])
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![eggs()]);
}

#[test]
fn or_where_raw_ors_sql_fragment() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .where_eq("id", "1")
            .or_where_raw("text = ?", ["eggs"])
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), eggs()]);
}

#[test]
fn when_true_applies_callback() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .when(true, |q| q.where_eq("id", "1"))
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk()]);
}

#[test]
fn when_false_skips_callback() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .when(false, |q| q.where_eq("id", "1"))
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), bread(), eggs()]);
}

#[test]
fn order_by_sorts_ascending() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().order_by("text"));

    // Assert
    assert_eq!(rows, vec![bread(), eggs(), milk()]);
}

#[test]
fn order_by_desc_sorts_descending() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().order_by_desc("id"));

    // Assert
    assert_eq!(rows, vec![eggs(), bread(), milk()]);
}

#[test]
fn order_by_raw_uses_sql_expression() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().order_by_raw("id DESC"));

    // Assert
    assert_eq!(rows, vec![eggs(), bread(), milk()]);
}

#[test]
fn latest_orders_column_descending() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().latest("id").limit(1));

    // Assert
    assert_eq!(rows, vec![eggs()]);
}

#[test]
fn oldest_orders_column_ascending() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().oldest("id").limit(1));

    // Assert
    assert_eq!(rows, vec![milk()]);
}

#[test]
fn in_random_order_still_returns_every_row() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let mut rows = run(&mut store, Todo::query().in_random_order());
    rows.sort_by(|a, b| a.id.cmp(&b.id));

    // Assert
    assert_eq!(rows, vec![milk(), bread(), eggs()]);
}

#[test]
fn reorder_clears_previous_order() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query()
            .order_by_desc("id")
            .reorder()
            .order_by("id"),
    );

    // Assert
    assert_eq!(rows, vec![milk(), bread(), eggs()]);
}

#[test]
fn reorder_desc_replaces_order() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query().order_by("text").reorder_desc("id").limit(1),
    );

    // Assert
    assert_eq!(rows, vec![eggs()]);
}

#[test]
fn limit_caps_row_count() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().order_by("id").limit(2));

    // Assert
    assert_eq!(rows, vec![milk(), bread()]);
}

#[test]
fn offset_skips_rows() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(
        &mut store,
        Todo::query().order_by("id").limit(1).offset(2),
    );

    // Assert
    assert_eq!(rows, vec![eggs()]);
}

#[test]
fn take_aliases_limit() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().order_by("id").take(1));

    // Assert
    assert_eq!(rows, vec![milk()]);
}

#[test]
fn skip_aliases_offset() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let rows = run(&mut store, Todo::query().order_by("id").take(1).skip(2));

    // Assert
    assert_eq!(rows, vec![eggs()]);
}

#[test]
fn offset_without_limit_is_error() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let err = store.watch(Todo::query().order_by("id").offset(1));

    // Assert
    match err {
        Err(Error::Sqlite(e)) => {
            assert!(e.to_string().contains("OFFSET requires LIMIT"));
        }
        Err(Error::Io(e)) => panic!("expected sqlite error, got io: {e}"),
        Ok(_) => panic!("expected sqlite error"),
    }
}

#[test]
fn for_page_is_one_indexed() {
    // Arrange
    let mut store = open();
    seed(&mut store);

    // Act
    let first = run(&mut store, Todo::query().order_by("id").for_page(1, 2));
    let second = run(&mut store, Todo::query().order_by("id").for_page(2, 2));

    // Assert
    assert_eq!(first, vec![milk(), bread()]);
    assert_eq!(second, vec![eggs()]);
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
