use homebase::rusqlite::{self, Row, Transaction, params};
use homebase::{Mutator, Table};

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

impl Table for Todo {
    const TABLE: &'static str = "todos";

    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: row.get(0)?,
            text: row.get(1)?,
            completed: row.get::<_, i64>(2)? != 0,
        })
    }
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
