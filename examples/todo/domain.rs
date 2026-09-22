use homebase::rusqlite::{self, Row, Transaction};
use homebase::{Mutator, Table};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Todo {
    pub id: String,
    pub text: String,
    pub completed: bool,
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

    fn values(&self) -> Vec<homebase::Bind> {
        vec![
            self.id.as_str().into(),
            self.text.as_str().into(),
            self.completed.into(),
        ]
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
                Todo::create(
                    tx,
                    &Todo {
                        id: id.clone(),
                        text: text.clone(),
                        completed: false,
                    },
                )?;
            }
            Event::Completed { id } => {
                Todo::where_eq("id", id.as_str())
                    .set("completed", true)
                    .update(tx)?;
            }
            Event::Uncompleted { id } => {
                Todo::where_eq("id", id.as_str())
                    .set("completed", false)
                    .update(tx)?;
            }
            Event::Deleted { id } => {
                Todo::where_eq("id", id.as_str()).delete(tx)?;
            }
        }
        Ok(())
    }
}
