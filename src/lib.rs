//! Local-first SQLite store. Consumers define schema (SQL migrations), models,
//! events, mutators, and live queries.

mod query;
mod store;

pub use rusqlite;
pub use query::{Bind, Select, Table};
pub use store::{Error, Live, Mutator, Notify, Query, Result, Store};
