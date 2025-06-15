use async_trait::async_trait;
use futures::future::join_all;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt::Display;
use std::io::Cursor;
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::str::from_utf8;
use std::{num::ParseIntError, path::PathBuf};
use thiserror::Error;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt};
use tokio::process::Command;
use tokio::{fs::File, io::AsyncRead};
use tracing::{debug, info, trace, warn};

use crate::hive::node::{
    Context, ExecuteStep, Goal, Push, SwitchToConfigurationGoal, push, should_apply_locally,
};
use crate::hive::steps::activate::get_elevation;
use crate::{HiveLibError, create_ssh_command};

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

async fn copy_buffer<T: AsyncWriteExt + Unpin>(
    reader: &mut T,
    buf: &[u8],
) -> Result<(), HiveLibError> {
    reader
        .write_all(buf)
        .await
        .map_err(HiveLibError::BufferOperationError)?;
    reader
        .flush()
        .await
        .map_err(HiveLibError::BufferOperationError)
}

async fn copy_buffers<T: AsyncWriteExt + Unpin>(
    reader: &mut T,
    bufs: Vec<Vec<u8>>,
) -> Result<(), HiveLibError> {
    for (index, buf) in bufs.iter().enumerate() {
        trace!("Pushing buf {}", index);
        copy_buffer(reader, buf).await?;
    }

    Ok(())
}

async fn process_key(key: &Key) -> Result<(key_agent::keys::Key, Vec<u8>), KeyError> {
    let mut reader = create_reader(&key.source).await?;

    let mut buf = Vec::new();

    reader
        .read_to_end(&mut buf)
        .await
        .expect("failed to read into buffer");

    let destination: PathBuf = [key.dest_dir.clone(), key.name.clone()].iter().collect();

    debug!(
        "Staging push to {}",
        destination.clone().into_os_string().into_string().unwrap()
    );

    Ok((
        key_agent::keys::Key {
            length: buf
                .len()
                .try_into()
                .expect("Failed to conver usize buf length to i32"),
            user: key.user.clone(),
            group: key.group.clone(),
            permissions: get_u32_permission(key)?,
            destination: destination.into_os_string().into_string().unwrap(),
        },
        buf,
    ))
}

pub struct KeysStep {
    pub moment: UploadKeyAt,
}
pub struct PushKeyAgentStep;

impl Display for KeysStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Upload key @ {:?}", self.moment)
    }
}

impl Display for PushKeyAgentStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Push the key agent")
    }
}

fn should_execute(moment: &UploadKeyAt, ctx: &Context) -> bool {
    if !key_step_should_execute(moment, ctx) {
        return false;
    }

    // excute step if node is not localhost
    // !should_apply_locally(ctx.node.allow_local_deployment, &ctx.name.to_string())
    true
}

#[async_trait]
impl ExecuteStep for KeysStep {
    fn should_execute(&self, ctx: &Context) -> bool {
        should_execute(&self.moment, ctx)
    }

    async fn execute(&self, ctx: &mut Context<'_>) -> Result<(), HiveLibError> {
        let agent_directory = ctx.state.key_agent_directory.as_ref().unwrap();

        let futures = ctx
            .node
            .keys
            .iter()
            .filter(|key| {
                self.moment == UploadKeyAt::AnyOpportunity
                    || (self.moment != UploadKeyAt::AnyOpportunity && key.upload_at != self.moment)
            })
            .map(|key| async move { process_key(key).await });

        let (keys, bufs): (Vec<key_agent::keys::Key>, Vec<Vec<u8>>) = join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, KeyError>>()
            .map_err(HiveLibError::KeyError)?
            .into_iter()
            .unzip();

        let msg = key_agent::keys::Keys { keys };

        trace!("Sending message {msg:?}");

        let buf = msg.encode_to_vec();

        let mut command =
            if should_apply_locally(ctx.node.allow_local_deployment, &ctx.name.to_string()) {
                warn!("Placing keys locally for node {0}", ctx.name);
                get_elevation("wire key agent")?;
                Command::new("sudo")
            } else {
                create_ssh_command(&ctx.node.target, true)
            };

        let mut child = command
            .args([
                format!("{agent_directory}/bin/key_agent"),
                buf.len().to_string(),
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped())
            .spawn()
            .map_err(HiveLibError::SpawnFailed)?;

        // take() stdin so it will be dropped out of block
        if let Some(mut stdin) = child.stdin.take() {
            trace!("Pushing msg");
            copy_buffer(&mut stdin, &buf).await?;
            copy_buffers(&mut stdin, bufs).await?;
        }

        let output = child
            .wait_with_output()
            .await
            .map_err(HiveLibError::SpawnFailed)?;

        if output.status.success() {
            info!("Successfully pushed keys to {}", ctx.name);
            trace!("Agent stdout: {}", String::from_utf8_lossy(&output.stdout));

            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);

        Err(HiveLibError::KeyCommandError(
            ctx.name.clone(),
            stderr
                .split('\n')
                .map(std::string::ToString::to_string)
                .collect(),
        ))
    }
}

#[async_trait]
impl ExecuteStep for PushKeyAgentStep {
    fn should_execute(&self, ctx: &Context) -> bool {
        should_execute(&UploadKeyAt::AnyOpportunity, ctx)
    }

    async fn execute(&self, ctx: &mut Context<'_>) -> Result<(), HiveLibError> {
        let arg_name = format!(
            "WIRE_KEY_AGENT_{platform}",
            platform = ctx.node.host_platform.replace('-', "_")
        );

        let agent_directory = match env::var_os(&arg_name) {
            Some(agent) => agent.into_string().unwrap(),
            None => panic!(
                "{arg_name} environment variable not set! \n
                Wire was not built with the ability to deploy keys to this platform. \n
                Please create an issue: https://github.com/wires-org/wire/issues/new?template=bug_report.md"
            ),
        };

        if !should_apply_locally(ctx.node.allow_local_deployment, &ctx.name.to_string()) {
            push(ctx.node, ctx.name, Push::Path(&agent_directory)).await?;
        }

        ctx.state.key_agent_directory = Some(agent_directory);

        Ok(())
    }
}
