use async_trait::async_trait;
use futures::future::join_all;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt::Display;
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::str::from_utf8;
use std::{io::Cursor, path::PathBuf};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::{fs::File, io::AsyncRead};
use tracing::{debug, info, trace, warn, Span};

use crate::hive::node::{should_apply_locally, Push};
use crate::{create_ssh_command, HiveLibError};

use super::node::{push, Context, ExecuteStep, Goal};

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

pub trait PushKeys {
    fn push_keys(
        self,
        target: UploadKeyAt,
        span: &Span,
    ) -> impl std::future::Future<Output = Result<(), HiveLibError>> + Send;
}

async fn create_reader(source: &'_ Source) -> Result<Pin<Box<dyn AsyncRead + Send + '_>>, Error> {
    match source {
        Source::Path(path) => Ok(Box::pin(File::open(path).await.map_err(Error::File)?)),
        Source::String(string) => Ok(Box::pin(Cursor::new(string))),
        Source::Command(args) => {
            let output = Command::new(args.first().ok_or(Error::Empty)?)
                .args(&args[1..])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(Error::CommandSpawnError)?
                .wait_with_output()
                .await
                .map_err(Error::CommandSpawnError)?;

            if output.status.success() {
                return Ok(Box::pin(Cursor::new(output.stdout)));
            }

            Err(Error::CommandError(
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

async fn process_key(key: &Key) -> Result<(agent::keys::Key, Vec<u8>), Error> {
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
        agent::keys::Key {
            length: buf
                .len()
                .try_into()
                .expect("Failed to conver usize buf length to i32"),
            user: key.user.clone(),
            group: key.group.clone(),
            permissions: u32::from_str_radix(&key.permissions, 8)
                .expect("Failed to convert octal string to u32"),
            destination: destination.into_os_string().into_string().unwrap(),
        },
        buf,
    ))
}

pub struct UploadKeyStep {
    pub moment: UploadKeyAt,
}
pub struct PushAgentStep;

impl Display for UploadKeyStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Upload key @ {:?}", self.moment)
    }
}

impl Display for PushAgentStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Push the agent")
    }
}

fn should_execute(moment: &UploadKeyAt, ctx: &Context) -> bool {
    if ctx.no_keys {
        return false;
    }
    if should_apply_locally(ctx.node.allow_local_deployment, &ctx.name.to_string()) {
        warn!(
            "SKIP STEP FOR {}: Pushing keys locally is unimplemented",
            ctx.name.to_string()
        );
        return false;
    }

    if *moment == UploadKeyAt::AnyOpportunity && matches!(ctx.goal, Goal::Keys) {
        return true;
    }

    matches!(
        ctx.goal,
        Goal::SwitchToConfiguration(super::node::SwitchToConfigurationGoal::Switch)
    )
}

#[async_trait]
impl ExecuteStep for UploadKeyStep {
    fn should_execute(&self, ctx: &Context) -> bool {
        should_execute(&self.moment, ctx)
    }

    async fn execute(&self, ctx: &mut Context<'_>) -> Result<(), HiveLibError> {
        let agent_directory = ctx.state.agent_directory.as_ref().unwrap();

        let futures = ctx
            .node
            .keys
            .iter()
            .filter(|key| {
                self.moment == UploadKeyAt::AnyOpportunity
                    || (self.moment != UploadKeyAt::AnyOpportunity && key.upload_at != self.moment)
            })
            .map(|key| async move { process_key(key).await });

        let (keys, bufs): (Vec<agent::keys::Key>, Vec<Vec<u8>>) = join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, Error>>()
            .map_err(HiveLibError::KeyError)?
            .into_iter()
            .unzip();

        let msg = agent::keys::Keys { keys };

        trace!("Sending message {msg:?}");

        let buf = msg.encode_to_vec();

        let mut command = create_ssh_command(&ctx.node.target, true);

        let mut child = command
            .args([
                format!("{agent_directory}/bin/agent"),
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
impl ExecuteStep for PushAgentStep {
    fn should_execute(&self, ctx: &Context) -> bool {
        should_execute(&UploadKeyAt::AnyOpportunity, ctx)
    }

    async fn execute(&self, ctx: &mut Context<'_>) -> Result<(), HiveLibError> {
        let agent_directory = match env::var_os("WIRE_AGENT") {
            Some(agent) => agent.into_string().unwrap(),
            None => panic!("WIRE_AGENT environment variable not set"),
        };

        push(ctx.node, ctx.name, Push::Path(&agent_directory)).await?;

        ctx.state.agent_directory = Some(agent_directory);

        Ok(())
    }
}
