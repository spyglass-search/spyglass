use std::fmt::{Display, Formatter, Result};

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

impl Display for OsName {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            OsName::MacOS => write!(f, "mac"),
            OsName::Windows => write!(f, "windows"),
            OsName::Linux => write!(f, "linux"),
            OsName::Unknown => write!(f, "Unknown"),
        }
    }
}

impl Default for RequestState {
    fn default() -> Self {
        Self::NotStarted
    }
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
        let user_agent = user_agent.to_lowercase();
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
