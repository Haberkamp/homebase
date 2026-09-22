use crate::domain::TodoQuery;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    Active,
    Completed,
}

impl Filter {
    pub fn all() -> [Filter; 3] {
        [Filter::All, Filter::Active, Filter::Completed]
    }

    pub fn query(self) -> TodoQuery {
        match self {
            Filter::All => TodoQuery::All,
            Filter::Active => TodoQuery::Active,
            Filter::Completed => TodoQuery::Completed,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Filter::All => "All",
            Filter::Active => "Active",
            Filter::Completed => "Completed",
        }
    }
}
