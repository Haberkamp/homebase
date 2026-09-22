//! Local-first SQLite store. Consumers define schema (SQL migrations), models,
//! events, mutators, and live queries.

mod query;
mod store;

pub use homebase_macros::Table;
pub use query::{Bind, Count, Exists, First, Select, Table};
pub use rusqlite;
pub use store::{Error, Live, Mutator, Notify, Query, Result, Store};
