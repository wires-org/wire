use async_trait::async_trait;
use futures::future::join_all;
use prost::Message;
use std::env;
use std::fmt::Display;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tracing::{debug, info, trace, warn};

use crate::hive::node::{Context, ExecuteStep, Push, push, should_apply_locally};
use crate::hive::steps::activate::get_elevation;
use crate::{HiveLibError, create_ssh_command};

use crate::hive::steps::keys::{
    Key, KeyError, UploadKeyAt, create_reader, get_u32_permission, key_step_should_execute,
};

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

pub struct UploadKeysToRemoteStep {
    pub moment: UploadKeyAt,
}
pub struct PushKeyAgentStep;

impl Display for UploadKeysToRemoteStep {
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
impl ExecuteStep for UploadKeysToRemoteStep {
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
