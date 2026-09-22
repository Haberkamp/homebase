//! Local-first SQLite store. Consumers define schema, models, events, mutators,
//! and live queries.

mod store;

pub use rusqlite;
pub use store::{Live, Mutator, Query, Store};
