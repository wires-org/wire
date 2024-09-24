use futures::future::join_all;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::env;
use std::pin::Pin;
use std::process::{ExitStatus, Stdio};
use std::str::from_utf8;
use std::{io::Cursor, path::PathBuf};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::{fs::File, io::AsyncRead};
use tracing::{debug, info, instrument, trace, Span};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use crate::hive::node::{Evaluatable, Push};
use crate::HiveLibError;

use super::node::{Name, Node};

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
#[serde(untagged)]
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
    All,
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

async fn process_key(name: &str, key: &Key) -> Result<(key_agent::keys::Key, Vec<u8>), Error> {
    let mut reader = create_reader(&key.source).await?;

    let mut buf = Vec::new();

    reader
        .read_to_end(&mut buf)
        .await
        .expect("failed to read into buffer");

    let destination: PathBuf = [key.dest_dir.clone(), name.into()].iter().collect();

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
            permissions: u32::from_str_radix(&key.permissions, 8)
                .expect("Failed to convert octal string to u32"),
            destination: destination.into_os_string().into_string().unwrap(),
        },
        buf,
    ))
}

impl PushKeys for (&Name, &Node) {
    #[instrument(skip_all)]
    async fn push_keys(self, target: UploadKeyAt, span: &Span) -> Result<(), HiveLibError> {
        let agent_directory = match env::var_os("WIRE_KEY_AGENT") {
            Some(agent) => agent.into_string().unwrap(),
            None => panic!("WIRE_KEY_AGENT environment variable not set"),
        };

        span.pb_inc_length(2);
        self.push(span, Push::Path(&agent_directory)).await?;
        span.pb_inc(1);

        let futures = self
            .1
            .keys
            .iter()
            .filter(|(_, key)| {
                target == UploadKeyAt::All
                    || (target != UploadKeyAt::All && key.upload_at != target)
            })
            .map(|(name, key)| async move { process_key(name, key).await });

        let (keys, bufs): (Vec<key_agent::keys::Key>, Vec<Vec<u8>>) = join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, Error>>()
            .map_err(HiveLibError::KeyError)?
            .into_iter()
            .unzip();

        let msg = key_agent::keys::Keys { keys };

        trace!("Sending message {msg:?}");

        let buf = msg.encode_to_vec();

        let mut command = Command::new("ssh");

        command.args([
            "-l",
            self.1.target.user.as_ref(),
            self.1.target.host.as_ref(),
        ]);

        if self.1.target.user != "root".into() {
            command.args(["sudo", "-H", "--"]);
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
            info!("Successfully pushed keys to {}", self.0);
            trace!("Agent stdout: {}", String::from_utf8_lossy(&output.stdout));

            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);

        Err(HiveLibError::KeyCommandError(
            self.0.clone(),
            stderr.split("\n").map(|s| s.to_string()).collect(),
        ))
    }
}
