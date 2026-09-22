use homebase::rusqlite::{self, Connection, Transaction, params};
use homebase::{Mutator, Query};

pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS todos (
    id TEXT PRIMARY KEY NOT NULL,
    text TEXT NOT NULL,
    completed INTEGER NOT NULL DEFAULT 0
);
";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Todo {
    pub id: String,
    pub text: String,
    pub completed: bool,
}

#[derive(Clone, Debug)]
pub enum Event {
    Created { id: String, text: String },
    Completed { id: String },
    Uncompleted { id: String },
    Deleted { id: String },
}

pub struct TodoMutator;

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
pub enum TodoQuery {
    Active,
    Completed,
    All,
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
            TodoQuery::All => "SELECT id, text, completed FROM todos ORDER BY id",
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
