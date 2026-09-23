use homebase::rusqlite::{self, Transaction};
use homebase::{Mutator, Table};

#[derive(Clone, Debug, PartialEq, Eq, Table)]
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
