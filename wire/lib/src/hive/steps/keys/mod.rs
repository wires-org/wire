use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::ExitStatus;
use thiserror::Error;

pub mod remote;

#[derive(Debug, Error)]
pub enum Error {
    #[error("error reading file")]
    File(#[source] std::io::Error),

    #[error("error spawning command")]
    CommandSpawnError(#[source] std::io::Error),

    #[error("key command failed with status {}: {}", .0,.1)]
    CommandError(ExitStatus, String),

    #[error("Command list empty")]
    Empty,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
#[serde(tag = "t", content = "c")]
pub enum Source {
    String(String),
    Path(PathBuf),
    Command(Vec<String>),
}

#[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
pub enum UploadKeyAt {
    #[serde(rename = "pre-activation")]
    PreActivation,
    #[serde(rename = "post-activation")]
    PostActivation,
    #[serde(skip)]
    AnyOpportunity,
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Key {
    pub name: String,
    #[serde(rename = "destDir")]
    pub dest_dir: String,
    pub path: PathBuf,
    pub group: String,
    pub user: String,
    pub permissions: String,
    pub source: Source,
    #[serde(rename = "uploadAt")]
    pub upload_at: UploadKeyAt,
}
