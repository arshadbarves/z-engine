#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("context packet requires nonempty {0}")]
    EmptyField(&'static str),
    #[error("duplicate context {kind} id: {id}")]
    DuplicateId { kind: &'static str, id: String },
    #[error("could not serialize context packet: {0}")]
    Serialization(#[from] serde_json::Error),
}
