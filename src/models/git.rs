#[allow(dead_code)]
pub enum Reference {
    Branch(String),
    Tag(String),
    Commit(String),
    Head,
}

/// Lightweight classification of a git reference, returned by `io::git::get_ref`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefKind {
    Tag,
    Branch,
    Commit,
}
