use gloo::utils::window;

pub enum OsName {
    MacOS,
    Windows,
    Linux,
    Unknown,
}

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

    pub fn in_progress(&self) -> bool {
        *self == Self::InProgress
    }
}

pub fn get_os() -> OsName {
    let nav = window().navigator();
    if let Ok(user_agent) = nav.user_agent() {
        if user_agent.contains("windows") {
            return OsName::Windows;
        } else if user_agent.contains("mac") {
            return OsName::MacOS;
        } else if user_agent.contains("linux") {
            return OsName::Linux;
        }
    }

    OsName::Unknown
}
