use std::marker::PhantomData;

use rusqlite::{Connection, Row, params_from_iter};

use crate::store::Query;

/// A row type that maps onto one SQLite table.
pub trait Table: Clone + PartialEq + 'static {
    const TABLE: &'static str;

    fn from_row(row: &Row<'_>) -> rusqlite::Result<Self>;

    fn query() -> Select<Self>
    where
        Self: Sized,
    {
        Select::new()
    }
}

/// Bound parameter for generated or raw SQL.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Bind {
    Null,
    Integer(i64),
    Text(String),
    Blob(Vec<u8>),
}

impl rusqlite::ToSql for Bind {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        match self {
            Self::Null => Ok(rusqlite::types::ToSqlOutput::Borrowed(
                rusqlite::types::ValueRef::Null,
            )),
            Self::Integer(v) => Ok((*v).into()),
            Self::Text(v) => Ok(v.as_str().into()),
            Self::Blob(v) => Ok(v.as_slice().into()),
        }
    }
}

impl From<bool> for Bind {
    fn from(value: bool) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i32> for Bind {
    fn from(value: i32) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<i64> for Bind {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<&str> for Bind {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

impl From<String> for Bind {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum BoolOp {
    And,
    Or,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Predicate {
    Cmp {
        column: String,
        op: String,
        value: Bind,
    },
    In {
        column: String,
        values: Vec<Bind>,
        not: bool,
    },
    Null {
        column: String,
        not: bool,
    },
    Between {
        column: String,
        min: Bind,
        max: Bind,
        not: bool,
    },
    Columns {
        left: String,
        op: String,
        right: String,
    },
    Raw {
        sql: String,
        binds: Vec<Bind>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct Clause {
    bool: BoolOp,
    predicate: Predicate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Dir {
    Asc,
    Desc,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Order {
    Column { name: String, dir: Dir },
    Raw(String),
    Random,
}

/// Fluent SELECT, inspired by Laravel's query builder.
///
/// `Select::sql` / `where_raw` are the SQLite escape hatches. Implementing
/// [`Query`] yourself is the fully custom path.
#[derive(Clone, Debug)]
pub struct Select<M> {
    table: &'static str,
    distinct: bool,
    columns: Option<Vec<String>>,
    clauses: Vec<Clause>,
    order: Vec<Order>,
    limit: Option<i64>,
    offset: Option<i64>,
    raw_sql: Option<(String, Vec<Bind>)>,
    _model: PhantomData<M>,
}

impl<M: Table> Select<M> {
    pub fn new() -> Self {
        Self {
            table: M::TABLE,
            distinct: false,
            columns: None,
            clauses: Vec::new(),
            order: Vec::new(),
            limit: None,
            offset: None,
            raw_sql: None,
            _model: PhantomData,
        }
    }

    /// Full custom SQLite. Bindings are `?1`, `?2`, …
    pub fn sql(
        sql: impl Into<String>,
        params: impl IntoIterator<Item = impl Into<Bind>>,
    ) -> Self {
        let mut q = Self::new();
        q.raw_sql = Some((
            sql.into(),
            params.into_iter().map(Into::into).collect(),
        ));
        q
    }

    fn and(self, predicate: Predicate) -> Self {
        self.push(BoolOp::And, predicate)
    }

    fn or(self, predicate: Predicate) -> Self {
        self.push(BoolOp::Or, predicate)
    }

    fn push(mut self, bool: BoolOp, predicate: Predicate) -> Self {
        self.clauses.push(Clause { bool, predicate });
        self
    }

    pub fn distinct(mut self) -> Self {
        self.distinct = true;
        self
    }

    pub fn select(mut self, columns: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.columns = Some(columns.into_iter().map(Into::into).collect());
        self
    }

    pub fn where_eq(self, column: impl Into<String>, value: impl Into<Bind>) -> Self {
        self.where_op(column, "=", value)
    }

    pub fn where_ne(self, column: impl Into<String>, value: impl Into<Bind>) -> Self {
        self.where_op(column, "!=", value)
    }

    pub fn where_lt(self, column: impl Into<String>, value: impl Into<Bind>) -> Self {
        self.where_op(column, "<", value)
    }

    pub fn where_lte(self, column: impl Into<String>, value: impl Into<Bind>) -> Self {
        self.where_op(column, "<=", value)
    }

    pub fn where_gt(self, column: impl Into<String>, value: impl Into<Bind>) -> Self {
        self.where_op(column, ">", value)
    }

    pub fn where_gte(self, column: impl Into<String>, value: impl Into<Bind>) -> Self {
        self.where_op(column, ">=", value)
    }

    pub fn or_where_eq(self, column: impl Into<String>, value: impl Into<Bind>) -> Self {
        self.or_where_op(column, "=", value)
    }

    pub fn or_where_ne(self, column: impl Into<String>, value: impl Into<Bind>) -> Self {
        self.or_where_op(column, "!=", value)
    }

    pub fn where_op(
        self,
        column: impl Into<String>,
        op: impl Into<String>,
        value: impl Into<Bind>,
    ) -> Self {
        self.and(Predicate::Cmp {
            column: column.into(),
            op: op.into(),
            value: value.into(),
        })
    }

    pub fn or_where_op(
        self,
        column: impl Into<String>,
        op: impl Into<String>,
        value: impl Into<Bind>,
    ) -> Self {
        self.or(Predicate::Cmp {
            column: column.into(),
            op: op.into(),
            value: value.into(),
        })
    }

    pub fn where_in(
        self,
        column: impl Into<String>,
        values: impl IntoIterator<Item = impl Into<Bind>>,
    ) -> Self {
        self.in_list(BoolOp::And, column, values, false)
    }

    pub fn where_not_in(
        self,
        column: impl Into<String>,
        values: impl IntoIterator<Item = impl Into<Bind>>,
    ) -> Self {
        self.in_list(BoolOp::And, column, values, true)
    }

    pub fn or_where_in(
        self,
        column: impl Into<String>,
        values: impl IntoIterator<Item = impl Into<Bind>>,
    ) -> Self {
        self.in_list(BoolOp::Or, column, values, false)
    }

    pub fn or_where_not_in(
        self,
        column: impl Into<String>,
        values: impl IntoIterator<Item = impl Into<Bind>>,
    ) -> Self {
        self.in_list(BoolOp::Or, column, values, true)
    }

    fn in_list(
        self,
        bool: BoolOp,
        column: impl Into<String>,
        values: impl IntoIterator<Item = impl Into<Bind>>,
        not: bool,
    ) -> Self {
        self.push(
            bool,
            Predicate::In {
                column: column.into(),
                values: values.into_iter().map(Into::into).collect(),
                not,
            },
        )
    }

    pub fn where_null(self, column: impl Into<String>) -> Self {
        self.and(Predicate::Null {
            column: column.into(),
            not: false,
        })
    }

    pub fn where_not_null(self, column: impl Into<String>) -> Self {
        self.and(Predicate::Null {
            column: column.into(),
            not: true,
        })
    }

    pub fn or_where_null(self, column: impl Into<String>) -> Self {
        self.or(Predicate::Null {
            column: column.into(),
            not: false,
        })
    }

    pub fn or_where_not_null(self, column: impl Into<String>) -> Self {
        self.or(Predicate::Null {
            column: column.into(),
            not: true,
        })
    }

    pub fn where_between(
        self,
        column: impl Into<String>,
        min: impl Into<Bind>,
        max: impl Into<Bind>,
    ) -> Self {
        self.between(BoolOp::And, column, min, max, false)
    }

    pub fn where_not_between(
        self,
        column: impl Into<String>,
        min: impl Into<Bind>,
        max: impl Into<Bind>,
    ) -> Self {
        self.between(BoolOp::And, column, min, max, true)
    }

    pub fn or_where_between(
        self,
        column: impl Into<String>,
        min: impl Into<Bind>,
        max: impl Into<Bind>,
    ) -> Self {
        self.between(BoolOp::Or, column, min, max, false)
    }

    fn between(
        self,
        bool: BoolOp,
        column: impl Into<String>,
        min: impl Into<Bind>,
        max: impl Into<Bind>,
        not: bool,
    ) -> Self {
        self.push(
            bool,
            Predicate::Between {
                column: column.into(),
                min: min.into(),
                max: max.into(),
                not,
            },
        )
    }

    pub fn where_like(self, column: impl Into<String>, pattern: impl Into<Bind>) -> Self {
        self.where_op(column, "LIKE", pattern)
    }

    pub fn where_not_like(self, column: impl Into<String>, pattern: impl Into<Bind>) -> Self {
        self.where_op(column, "NOT LIKE", pattern)
    }

    pub fn or_where_like(self, column: impl Into<String>, pattern: impl Into<Bind>) -> Self {
        self.or_where_op(column, "LIKE", pattern)
    }

    pub fn or_where_not_like(self, column: impl Into<String>, pattern: impl Into<Bind>) -> Self {
        self.or_where_op(column, "NOT LIKE", pattern)
    }

    pub fn where_column(
        self,
        left: impl Into<String>,
        op: impl Into<String>,
        right: impl Into<String>,
    ) -> Self {
        self.and(Predicate::Columns {
            left: left.into(),
            op: op.into(),
            right: right.into(),
        })
    }

    pub fn or_where_column(
        self,
        left: impl Into<String>,
        op: impl Into<String>,
        right: impl Into<String>,
    ) -> Self {
        self.or(Predicate::Columns {
            left: left.into(),
            op: op.into(),
            right: right.into(),
        })
    }

    pub fn where_raw(
        self,
        sql: impl Into<String>,
        params: impl IntoIterator<Item = impl Into<Bind>>,
    ) -> Self {
        self.and(Predicate::Raw {
            sql: sql.into(),
            binds: params.into_iter().map(Into::into).collect(),
        })
    }

    pub fn or_where_raw(
        self,
        sql: impl Into<String>,
        params: impl IntoIterator<Item = impl Into<Bind>>,
    ) -> Self {
        self.or(Predicate::Raw {
            sql: sql.into(),
            binds: params.into_iter().map(Into::into).collect(),
        })
    }

    pub fn when(self, cond: bool, f: impl FnOnce(Self) -> Self) -> Self {
        if cond { f(self) } else { self }
    }

    pub fn order_by(mut self, column: impl Into<String>) -> Self {
        self.order.push(Order::Column {
            name: column.into(),
            dir: Dir::Asc,
        });
        self
    }

    pub fn order_by_desc(mut self, column: impl Into<String>) -> Self {
        self.order.push(Order::Column {
            name: column.into(),
            dir: Dir::Desc,
        });
        self
    }

    pub fn order_by_raw(mut self, sql: impl Into<String>) -> Self {
        self.order.push(Order::Raw(sql.into()));
        self
    }

    pub fn latest(self, column: impl Into<String>) -> Self {
        self.order_by_desc(column)
    }

    pub fn oldest(self, column: impl Into<String>) -> Self {
        self.order_by(column)
    }

    pub fn in_random_order(mut self) -> Self {
        self.order.push(Order::Random);
        self
    }

    pub fn reorder(mut self) -> Self {
        self.order.clear();
        self
    }

    pub fn reorder_desc(self, column: impl Into<String>) -> Self {
        self.reorder().order_by_desc(column)
    }

    pub fn limit(mut self, n: i64) -> Self {
        self.limit = Some(n);
        self
    }

    /// Skip `n` rows. SQLite requires a [`limit`](Self::limit) (or [`take`](Self::take)) as well.
    pub fn offset(mut self, n: i64) -> Self {
        self.offset = Some(n);
        self
    }

    pub fn take(self, n: i64) -> Self {
        self.limit(n)
    }

    /// Alias for [`offset`](Self::offset). Requires a limit.
    pub fn skip(self, n: i64) -> Self {
        self.offset(n)
    }

    pub fn for_page(self, page: i64, per_page: i64) -> Self {
        let page = page.max(1);
        self.offset((page - 1) * per_page).limit(per_page)
    }

    fn compile(&self) -> rusqlite::Result<(String, Vec<Bind>)> {
        if let Some((sql, binds)) = &self.raw_sql {
            return Ok((sql.clone(), binds.clone()));
        }

        let distinct = if self.distinct { "DISTINCT " } else { "" };
        let cols = match &self.columns {
            None => "*".to_string(),
            Some(columns) if columns.is_empty() => "*".to_string(),
            Some(columns) => columns
                .iter()
                .map(|c| quote_ident(c))
                .collect::<rusqlite::Result<Vec<_>>>()?
                .join(", "),
        };
        let mut sql = format!("SELECT {distinct}{cols} FROM {}", quote_ident(self.table)?);
        let mut binds = Vec::new();

        if !self.clauses.is_empty() {
            sql.push_str(" WHERE ");
            for (i, clause) in self.clauses.iter().enumerate() {
                if i > 0 {
                    sql.push_str(match clause.bool {
                        BoolOp::And => " AND ",
                        BoolOp::Or => " OR ",
                    });
                }
                sql.push_str(&compile_predicate(&clause.predicate, &mut binds)?);
            }
        }

        if !self.order.is_empty() {
            sql.push_str(" ORDER BY ");
            for (i, order) in self.order.iter().enumerate() {
                if i > 0 {
                    sql.push_str(", ");
                }
                match order {
                    Order::Column { name, dir } => {
                        sql.push_str(&quote_ident(name)?);
                        sql.push_str(match dir {
                            Dir::Asc => " ASC",
                            Dir::Desc => " DESC",
                        });
                    }
                    Order::Raw(raw) => sql.push_str(raw),
                    Order::Random => sql.push_str("RANDOM()"),
                }
            }
        }

        if self.offset.is_some() && self.limit.is_none() {
            return Err(rusqlite::Error::InvalidParameterName(
                "OFFSET requires LIMIT".into(),
            ));
        }
        if let Some(limit) = self.limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = self.offset {
            sql.push_str(&format!(" OFFSET {offset}"));
        }

        Ok((sql, binds))
    }
}

impl<M> PartialEq for Select<M> {
    fn eq(&self, other: &Self) -> bool {
        self.table == other.table
            && self.distinct == other.distinct
            && self.columns == other.columns
            && self.clauses == other.clauses
            && self.order == other.order
            && self.limit == other.limit
            && self.offset == other.offset
            && self.raw_sql == other.raw_sql
    }
}

impl<M> Eq for Select<M> {}

impl<M> std::hash::Hash for Select<M> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.table.hash(state);
        self.distinct.hash(state);
        self.columns.hash(state);
        self.clauses.hash(state);
        self.order.hash(state);
        self.limit.hash(state);
        self.offset.hash(state);
        self.raw_sql.hash(state);
    }
}

impl<M: Table> Default for Select<M> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: Table> Query for Select<M> {
    type Row = M;

    fn execute(&self, conn: &Connection) -> rusqlite::Result<Vec<M>> {
        let (sql, binds) = self.compile()?;
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(binds), M::from_row)?;
        rows.collect()
    }
}

fn quote_ident(name: &str) -> rusqlite::Result<String> {
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(format!("\"{name}\""))
    } else {
        Err(rusqlite::Error::InvalidParameterName(name.into()))
    }
}

fn compile_predicate(pred: &Predicate, binds: &mut Vec<Bind>) -> rusqlite::Result<String> {
    match pred {
        Predicate::Cmp { column, op, value } => {
            let op = op.to_uppercase();
            const OPS: &[&str] = &[
                "=", "!=", "<>", "<", ">", "<=", ">=", "LIKE", "NOT LIKE", "GLOB",
            ];
            if !OPS.contains(&op.as_str()) {
                return Err(rusqlite::Error::InvalidParameterName(op));
            }
            binds.push(value.clone());
            Ok(format!("{} {op} ?", quote_ident(column)?))
        }
        Predicate::In {
            column,
            values,
            not,
        } => {
            if values.is_empty() {
                return Ok(if *not { "1".into() } else { "0".into() });
            }
            let placeholders = vec!["?"; values.len()].join(", ");
            binds.extend(values.iter().cloned());
            let not = if *not { " NOT" } else { "" };
            Ok(format!(
                "{}{not} IN ({placeholders})",
                quote_ident(column)?
            ))
        }
        Predicate::Null { column, not } => {
            let op = if *not { "IS NOT NULL" } else { "IS NULL" };
            Ok(format!("{} {op}", quote_ident(column)?))
        }
        Predicate::Between {
            column,
            min,
            max,
            not,
        } => {
            binds.push(min.clone());
            binds.push(max.clone());
            let not = if *not { " NOT" } else { "" };
            Ok(format!("{}{not} BETWEEN ? AND ?", quote_ident(column)?))
        }
        Predicate::Columns { left, op, right } => {
            let op = op.to_uppercase();
            const OPS: &[&str] = &["=", "!=", "<>", "<", ">", "<=", ">="];
            if !OPS.contains(&op.as_str()) {
                return Err(rusqlite::Error::InvalidParameterName(op));
            }
            Ok(format!(
                "{} {op} {}",
                quote_ident(left)?,
                quote_ident(right)?
            ))
        }
        Predicate::Raw { sql, binds: raw } => {
            binds.extend(raw.iter().cloned());
            Ok(format!("({sql})"))
        }
    }
}
