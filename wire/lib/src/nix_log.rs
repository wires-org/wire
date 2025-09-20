use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::fmt::{Debug, Display};
use tracing::{Level as tracing_level, error, event, info};

// static DIGEST_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9a-z]{32}").unwrap());

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "action")]
pub enum Action {
    #[serde(rename = "msg", alias = "start")]
    Message {
        level: Level,
        #[serde(rename = "msg", alias = "text")]
        message: Option<String>,
    },
    #[serde(rename = "stop", alias = "result")]
    Stop,
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug)]
#[repr(u8)]
pub enum Level {
    Error = 0,
    Warn = 1,
    Notice = 2,
    Info = 3,
    Talkative = 4,
    Chatty = 5,
    Debug = 6,
    Vomit = 7,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Internal {
    #[serde(flatten)]
    pub action: Action,
}

#[derive(Debug)]
pub enum NixLog {
    Internal(Internal),
    Raw(String),
    RawError(String),
}

pub(crate) trait Trace {
    fn trace(&self);
}

impl Internal {
    pub fn get_errorish_message(self) -> Option<String> {
        if let Action::Message {
            level: Level::Error | Level::Warn | Level::Notice,
            message,
        } = self.action
        {
            return message;
        }

        None
    }
}

impl Trace for Internal {
    fn trace(&self) {
        match &self.action {
            Action::Message { level, message } => {
                let text = match message {
                    Some(text) if text.is_empty() => return,
                    None => return,
                    Some(text) => text,
                };

                match level {
                    Level::Info => event!(tracing_level::INFO, "{text}"),
                    Level::Warn | Level::Notice => event!(tracing_level::WARN, "{text}"),
                    Level::Error => event!(tracing_level::ERROR, "{text}"),
                    Level::Debug => event!(tracing_level::DEBUG, "{text}"),
                    Level::Vomit | Level::Talkative | Level::Chatty => {
                        event!(tracing_level::TRACE, "{text}");
                    }
                }
            }
            Action::Stop => {}
        }
    }
}

impl Trace for NixLog {
    fn trace(&self) {
        match self {
            NixLog::Internal(line) => {
                line.trace();

                // tracing_indicatif::span_ext::IndicatifSpanExt::pb_set_message(
                //     &Span::current(),
                //     &DIGEST_RE.replace_all(&line.to_string(), "…"),
                // );
            }
            NixLog::Raw(line) => info!("{line}"),
            NixLog::RawError(line) => error!("{line}"),
        }
    }
}

impl Display for Internal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.action {
            Action::Message { level, message } => {
                write!(
                    f,
                    "{level:?}: {}",
                    match message {
                        Some(message) => message,
                        None => "Nix log without text",
                    }
                )
            }
            Action::Stop => write!(f, ""),
        }
    }
}

impl Display for NixLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            NixLog::Internal(line) => Display::fmt(&line, f),
            NixLog::Raw(line) | NixLog::RawError(line) => Display::fmt(&line, f),
        }
    }
}
