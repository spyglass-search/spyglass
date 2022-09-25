#[derive(Debug, PartialEq, Eq)]
pub enum RequestState {
    NotStarted,
    InProgress,
    Finished,
    Error,
}

impl RequestState {
    pub fn is_done(&self) -> bool {
        *self == Self::Finished || *self == Self::Error
    }
}
