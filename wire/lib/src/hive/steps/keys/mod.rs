use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::str::from_utf8;
use std::{num::ParseIntError, path::PathBuf};
use thiserror::Error;
use tokio::process::Command;
use tokio::{fs::File, io::AsyncRead};

use crate::hive::node::{Goal, SwitchToConfigurationGoal};

pub mod remote;

#[derive(Debug, Error)]
pub enum KeyError {
    #[error("error reading file")]
    File(#[source] std::io::Error),

    #[error("error spawning command")]
    CommandSpawnError(#[source] std::io::Error),

    #[error("key command failed with status {}: {}", .0,.1)]
    CommandError(ExitStatus, String),

    #[error("Command list empty")]
    Empty,

    #[error("Failed to parse key permissions")]
    ParseKeyPermissions(#[source] ParseIntError),

    #[error("failed to place a key locally")]
    FailedToPlaceLocalKey(#[source] std::io::Error),
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

fn key_step_should_execute(moment: &UploadKeyAt, ctx: &crate::hive::node::Context) -> bool {
    if ctx.no_keys {
        return false;
    }

    if *moment == UploadKeyAt::AnyOpportunity && matches!(ctx.goal, Goal::Keys) {
        return true;
    }

    matches!(
        ctx.goal,
        Goal::SwitchToConfiguration(SwitchToConfigurationGoal::Switch)
    )
}

fn get_u32_permission(key: &Key) -> Result<u32, KeyError> {
    u32::from_str_radix(&key.permissions, 8).map_err(KeyError::ParseKeyPermissions)
}

async fn create_reader(
    source: &'_ Source,
) -> Result<Pin<Box<dyn AsyncRead + Send + '_>>, KeyError> {
    match source {
        Source::Path(path) => Ok(Box::pin(File::open(path).await.map_err(KeyError::File)?)),
        Source::String(string) => Ok(Box::pin(Cursor::new(string))),
        Source::Command(args) => {
            let output = Command::new(args.first().ok_or(KeyError::Empty)?)
                .args(&args[1..])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(KeyError::CommandSpawnError)?
                .wait_with_output()
                .await
                .map_err(KeyError::CommandSpawnError)?;

            if output.status.success() {
                return Ok(Box::pin(Cursor::new(output.stdout)));
            }

            Err(KeyError::CommandError(
                output.status,
                from_utf8(&output.stderr).unwrap().to_string(),
            ))
        }
    }
}
