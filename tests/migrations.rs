use homestead::rusqlite::Connection;
use homestead::{Error, Mutator, Store};
use tempfile::tempdir;

const CREATE_TODOS: &str = "
CREATE TABLE todos (
    id TEXT PRIMARY KEY NOT NULL,
    text TEXT NOT NULL,
    completed INTEGER NOT NULL DEFAULT 0
);
";

struct Noop;

impl Mutator<()> for Noop {
    fn apply(
        &self,
        _tx: &homestead::rusqlite::Transaction<'_>,
        _event: &(),
    ) -> homestead::rusqlite::Result<()> {
        Ok(())
    }
}

fn write_sql(dir: &std::path::Path, name: &str, sql: &str) {
    std::fs::write(dir.join(name), sql).unwrap();
}

fn applied_names(db: &std::path::Path) -> Vec<String> {
    let conn = Connection::open(db).unwrap();
    let mut stmt = conn
        .prepare("SELECT name FROM migrations ORDER BY name")
        .unwrap();
    stmt.query_map([], |row| row.get(0))
        .unwrap()
        .map(|r| r.unwrap())
        .collect()
}

fn has_column(db: &std::path::Path, column: &str) -> bool {
    let conn = Connection::open(db).unwrap();
    let mut stmt = conn.prepare("PRAGMA table_info(todos)").unwrap();
    let names: Vec<String> = stmt
        .query_map([], |row| row.get(1))
        .unwrap()
        .map(|r| r.unwrap())
        .collect();
    names.iter().any(|n| n == column)
}

fn has_table(db: &std::path::Path, table: &str) -> bool {
    let conn = Connection::open(db).unwrap();
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [table],
            |row| row.get(0),
        )
        .unwrap();
    n > 0
}

#[test]
fn applies_sql_files_in_filename_order() {
    // Arrange
    let root = tempdir().unwrap();
    let migrations = root.path().join("migrations");
    std::fs::create_dir(&migrations).unwrap();
    write_sql(
        &migrations,
        "002_second.sql",
        "CREATE TABLE second (id INTEGER);",
    );
    write_sql(
        &migrations,
        "001_first.sql",
        "CREATE TABLE first (id INTEGER);",
    );
    let db = root.path().join("app.db");

    // Act
    Store::open(&db, &migrations, Noop).unwrap();

    // Assert
    assert!(has_table(&db, "first"));
    assert!(has_table(&db, "second"));
    assert_eq!(
        applied_names(&db),
        vec!["001_first.sql".to_string(), "002_second.sql".to_string()]
    );
}

#[test]
fn skips_already_applied_files() {
    // Arrange
    let root = tempdir().unwrap();
    let migrations = root.path().join("migrations");
    std::fs::create_dir(&migrations).unwrap();
    write_sql(&migrations, "001_todos.sql", CREATE_TODOS);
    let db = root.path().join("app.db");
    Store::open(&db, &migrations, Noop).unwrap();

    // Act
    Store::open(&db, &migrations, Noop).unwrap();

    // Assert
    assert_eq!(applied_names(&db), vec!["001_todos.sql".to_string()]);
}

#[test]
fn applies_new_file_on_later_open() {
    // Arrange
    let root = tempdir().unwrap();
    let migrations = root.path().join("migrations");
    std::fs::create_dir(&migrations).unwrap();
    write_sql(&migrations, "001_todos.sql", CREATE_TODOS);
    let db = root.path().join("app.db");
    Store::open(&db, &migrations, Noop).unwrap();
    write_sql(
        &migrations,
        "002_notes.sql",
        "ALTER TABLE todos ADD COLUMN notes TEXT NOT NULL DEFAULT '';",
    );

    // Act
    Store::open(&db, &migrations, Noop).unwrap();

    // Assert
    assert!(has_column(&db, "notes"));
    assert_eq!(
        applied_names(&db),
        vec!["001_todos.sql".to_string(), "002_notes.sql".to_string()]
    );
}

#[test]
fn ignores_non_sql_files() {
    // Arrange
    let root = tempdir().unwrap();
    let migrations = root.path().join("migrations");
    std::fs::create_dir(&migrations).unwrap();
    write_sql(&migrations, "001_todos.sql", CREATE_TODOS);
    std::fs::write(migrations.join("README.md"), "skip me").unwrap();
    let db = root.path().join("app.db");

    // Act
    Store::open(&db, &migrations, Noop).unwrap();

    // Assert
    assert_eq!(applied_names(&db), vec!["001_todos.sql".to_string()]);
}

#[test]
fn empty_directory_opens() {
    // Arrange
    let root = tempdir().unwrap();
    let migrations = root.path().join("migrations");
    std::fs::create_dir(&migrations).unwrap();

    // Act
    let result = Store::open(":memory:", &migrations, Noop);

    // Assert
    result.unwrap();
}

#[test]
fn missing_directory_is_io_error() {
    // Arrange
    let missing = tempdir().unwrap().path().join("nope");

    // Act
    let err = match Store::<()>::open(":memory:", &missing, Noop) {
        Err(e) => e,
        Ok(_) => panic!("expected io error"),
    };

    // Assert
    match err {
        Error::Io(_) => {}
        Error::Sqlite(e) => panic!("expected io error, got sqlite: {e}"),
    }
}

#[test]
fn invalid_sql_is_sqlite_error() {
    // Arrange
    let root = tempdir().unwrap();
    let migrations = root.path().join("migrations");
    std::fs::create_dir(&migrations).unwrap();
    write_sql(&migrations, "001_bad.sql", "not valid sql");

    // Act
    let err = match Store::<()>::open(":memory:", &migrations, Noop) {
        Err(e) => e,
        Ok(_) => panic!("expected sqlite error"),
    };

    // Assert
    match err {
        Error::Sqlite(e) => assert!(e.to_string().contains("syntax")),
        Error::Io(e) => panic!("expected sqlite error, got io: {e}"),
    }
}

#[test]
fn failed_migration_is_not_recorded() {
    // Arrange
    let root = tempdir().unwrap();
    let migrations = root.path().join("migrations");
    std::fs::create_dir(&migrations).unwrap();
    write_sql(&migrations, "001_todos.sql", CREATE_TODOS);
    write_sql(&migrations, "002_bad.sql", "not valid sql");
    let db = root.path().join("app.db");

    // Act
    let err = match Store::<()>::open(&db, &migrations, Noop) {
        Err(e) => e,
        Ok(_) => panic!("expected sqlite error"),
    };

    // Assert
    match err {
        Error::Sqlite(_) => {}
        Error::Io(e) => panic!("expected sqlite error, got io: {e}"),
    }
    assert!(has_table(&db, "todos"));
    assert_eq!(applied_names(&db), vec!["001_todos.sql".to_string()]);
}
