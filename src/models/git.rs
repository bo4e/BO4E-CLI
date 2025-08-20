pub enum Reference {
    Branch(String),
    Tag(String),
    Commit(String),
    Head,
}
