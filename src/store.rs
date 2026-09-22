use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::path::Path;
use std::rc::Rc;

use rusqlite::{Connection, Transaction};

/// Store operation failure. Query and mutator SQLite errors surface as `Sqlite`.
#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sqlite(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(err) => Some(err),
        }
    }
}

impl From<rusqlite::Error> for Error {
    fn from(err: rusqlite::Error) -> Self {
        Self::Sqlite(err)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Something `commit` can notify after watches refresh. GPUI `Context` notifies
/// the view; tests pass `&mut ()`.
pub trait Notify {
    fn notify(&mut self);
}

impl Notify for () {
    fn notify(&mut self) {}
}

impl<T: 'static> Notify for gpui::Context<'_, T> {
    fn notify(&mut self) {
        gpui::Context::notify(self);
    }
}

/// Maps a consumer event onto SQLite writes.
pub trait Mutator<E> {
    fn apply(&self, tx: &Transaction<'_>, event: &E) -> rusqlite::Result<()>;
}

impl<E, F> Mutator<E> for F
where
    F: Fn(&Transaction<'_>, &E) -> rusqlite::Result<()>,
{
    fn apply(&self, tx: &Transaction<'_>, event: &E) -> rusqlite::Result<()> {
        self(tx, event)
    }
}

/// A live query defined by the consumer.
pub trait Query: Clone + Eq + Hash + 'static {
    type Row: Clone + PartialEq + 'static;

    fn execute(&self, conn: &Connection) -> rusqlite::Result<Vec<Self::Row>>;
}

struct QueryKey {
    type_id: TypeId,
    hash: u64,
    value: Box<dyn Any>,
    eq: fn(&dyn Any, &dyn Any) -> bool,
    clone_value: fn(&dyn Any) -> Box<dyn Any>,
}

impl QueryKey {
    fn new<Q: Query>(query: &Q) -> Self {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        query.hash(&mut hasher);
        Self {
            type_id: TypeId::of::<Q>(),
            hash: hasher.finish(),
            value: Box::new(query.clone()),
            eq: typed_eq::<Q>,
            clone_value: clone_query::<Q>,
        }
    }
}

fn typed_eq<Q: Query>(a: &dyn Any, b: &dyn Any) -> bool {
    a.downcast_ref::<Q>() == b.downcast_ref::<Q>()
}

fn clone_query<Q: Query>(value: &dyn Any) -> Box<dyn Any> {
    Box::new(value.downcast_ref::<Q>().expect("query type").clone())
}

impl Clone for QueryKey {
    fn clone(&self) -> Self {
        Self {
            type_id: self.type_id,
            hash: self.hash,
            value: (self.clone_value)(&*self.value),
            eq: self.eq,
            clone_value: self.clone_value,
        }
    }
}

impl PartialEq for QueryKey {
    fn eq(&self, other: &Self) -> bool {
        self.type_id == other.type_id && (self.eq)(&*self.value, &*other.value)
    }
}

impl Eq for QueryKey {}

impl Hash for QueryKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.type_id.hash(state);
        self.hash.hash(state);
    }
}

struct Subscription {
    last: Box<dyn Any>,
    execute: Box<dyn Fn(&Connection) -> Result<Box<dyn Any>>>,
    rows_eq: fn(&dyn Any, &dyn Any) -> bool,
    callbacks: Vec<Box<dyn FnMut(&dyn Any)>>,
}

/// In-memory snapshot of a watched query. `commit` keeps this current; the
/// consumer does not re-query after writes.
pub struct Live<Q: Query> {
    rows: Rc<RefCell<Vec<Q::Row>>>,
    _query: PhantomData<Q>,
}

impl<Q: Query> Live<Q> {
    pub fn rows(&self) -> Vec<Q::Row> {
        self.rows.borrow().clone()
    }
}

/// Local-first SQLite store. Schema, events, mutators, and queries belong to
/// the consumer.
pub struct Store<E> {
    conn: Connection,
    mutator: Box<dyn Mutator<E>>,
    subscriptions: HashMap<QueryKey, Subscription>,
}

impl<E> Store<E> {
    pub fn open(
        path: impl AsRef<Path>,
        schema_sql: &str,
        mutator: impl Mutator<E> + 'static,
    ) -> Result<Self> {
        let path = path.as_ref();
        let conn = if path.as_os_str() == ":memory:" {
            Connection::open_in_memory()
        } else {
            Connection::open(path)
        }?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        if !schema_sql.trim().is_empty() {
            conn.execute_batch(schema_sql)?;
        }

        Ok(Self {
            conn,
            mutator: Box::new(mutator),
            subscriptions: HashMap::new(),
        })
    }

    pub fn commit(&mut self, event: E, cx: &mut impl Notify) -> Result<()> {
        {
            let tx = self.conn.transaction()?;
            self.mutator.apply(&tx, &event)?;
            tx.commit()?;
        }
        self.refresh()?;
        cx.notify();
        Ok(())
    }

    pub fn query<Q: Query>(&mut self, query: Q) -> Result<Vec<Q::Row>> {
        Ok(query.execute(&self.conn)?)
    }

    /// Watch a query. After each `commit`, `Live::rows` matches SQLite without a
    /// follow-up `query`.
    pub fn watch<Q: Query>(&mut self, query: Q) -> Result<Live<Q>> {
        let rows = Rc::new(RefCell::new(query.execute(&self.conn)?));
        let rows_for_cb = rows.clone();
        self.subscribe(query, move |next| {
            *rows_for_cb.borrow_mut() = next.to_vec();
        })?;
        Ok(Live {
            rows,
            _query: PhantomData,
        })
    }

    /// Fires only when the result *changes* (not on subscribe, not on equal re-run).
    pub fn subscribe<Q: Query>(
        &mut self,
        query: Q,
        mut on_change: impl FnMut(&[Q::Row]) + 'static,
    ) -> Result<()> {
        let snapshot = query.execute(&self.conn)?;
        let key = QueryKey::new(&query);
        let entry = self.subscriptions.entry(key).or_insert_with(|| {
            let q = query.clone();
            Subscription {
                last: Box::new(snapshot.clone()),
                execute: Box::new(move |conn| {
                    let rows = q.execute(conn)?;
                    Ok(Box::new(rows) as Box<dyn Any>)
                }),
                rows_eq: vec_eq::<Q::Row>,
                callbacks: Vec::new(),
            }
        });
        if entry.callbacks.is_empty() {
            entry.last = Box::new(snapshot);
        }
        entry.callbacks.push(Box::new(move |any: &dyn Any| {
            let rows = any.downcast_ref::<Vec<Q::Row>>().expect("query row type");
            on_change(rows);
        }));
        Ok(())
    }

    fn refresh(&mut self) -> Result<()> {
        let keys: Vec<QueryKey> = self.subscriptions.keys().cloned().collect();

        for key in keys {
            let next = {
                let sub = self
                    .subscriptions
                    .get(&key)
                    .expect("subscribed query");
                (sub.execute)(&self.conn)?
            };
            let sub = self
                .subscriptions
                .get_mut(&key)
                .expect("subscribed query");
            if (sub.rows_eq)(&*next, &*sub.last) {
                continue;
            }
            sub.last = next;
            let last = &*sub.last;
            for cb in &mut sub.callbacks {
                cb(last);
            }
        }
        Ok(())
    }
}

fn vec_eq<R: PartialEq + 'static>(a: &dyn Any, b: &dyn Any) -> bool {
    a.downcast_ref::<Vec<R>>() == b.downcast_ref::<Vec<R>>()
}
