use futures::future::join_all;
use prost::Message;
use serde::{Deserialize, Serialize};
use std::env;
use std::fmt::Display;
use std::io::Cursor;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::str::from_utf8;
use tokio::io::AsyncReadExt as _;
use tokio::process::Command;
use tokio::{fs::File, io::AsyncRead};
use tracing::{debug, trace};

use crate::HiveLibError;
use crate::commands::common::push;
use crate::commands::{ChildOutputMode, WireCommand, WireCommandChip, get_elevated_command};
use crate::errors::KeyError;
use crate::hive::node::{
    Context, ExecuteStep, Goal, Push, SwitchToConfigurationGoal, should_apply_locally,
};

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
    NoFilter,
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
    #[serde(default)]
    pub environment: im::HashMap<String, String>,
}

fn get_u32_permission(key: &Key) -> Result<u32, KeyError> {
    u32::from_str_radix(&key.permissions, 8).map_err(KeyError::ParseKeyPermissions)
}

async fn create_reader(key: &'_ Key) -> Result<Pin<Box<dyn AsyncRead + Send + '_>>, KeyError> {
    match &key.source {
        Source::Path(path) => Ok(Box::pin(File::open(path).await.map_err(KeyError::File)?)),
        Source::String(string) => Ok(Box::pin(Cursor::new(string))),
        Source::Command(args) => {
            let output = Command::new(args.first().ok_or(KeyError::Empty)?)
                .args(&args[1..])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .envs(key.environment.clone())
                .spawn()
                .map_err(|err| KeyError::CommandSpawnError {
                    error: err,
                    command: args.join(" "),
                    command_span: Some((0..args.first().unwrap().len()).into()),
                })?
                .wait_with_output()
                .await
                .map_err(|err| KeyError::CommandResolveError {
                    error: err,
                    command: args.join(" "),
                })?;

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

async fn process_key(key: &Key) -> Result<(key_agent::keys::Key, Vec<u8>), KeyError> {
    let mut reader = create_reader(key).await?;

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

#[derive(Debug, PartialEq)]
pub struct Keys {
    pub filter: UploadKeyAt,
}
#[derive(Debug, PartialEq)]
pub struct PushKeyAgent;

impl Display for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Upload key @ {:?}", self.filter)
    }
}

impl Display for PushKeyAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Push the key agent")
    }
}

impl ExecuteStep for Keys {
    fn should_execute(&self, ctx: &Context) -> bool {
        if ctx.no_keys {
            return false;
        }

        // should execute if no filter, and the goal is keys.
        // otherwise, only execute if the goal is switch and non-nofilter
        matches!(
            (&self.filter, &ctx.goal),
            (UploadKeyAt::NoFilter, Goal::Keys)
                | (
                    UploadKeyAt::PreActivation | UploadKeyAt::PostActivation,
                    Goal::SwitchToConfiguration(SwitchToConfigurationGoal::Switch)
                )
        )
    }

    async fn execute(&self, ctx: &mut Context<'_>) -> Result<(), HiveLibError> {
        let agent_directory = ctx.state.key_agent_directory.as_ref().unwrap();

        let futures = ctx
            .node
            .keys
            .iter()
            .filter(|key| {
                self.filter == UploadKeyAt::NoFilter
                    || (self.filter != UploadKeyAt::NoFilter && key.upload_at != self.filter)
            })
            .map(|key| async move {
                process_key(key)
                    .await
                    .map_err(|err| HiveLibError::KeyError(key.name.clone(), err))
            });

        let (keys, bufs): (Vec<key_agent::keys::Key>, Vec<Vec<u8>>) = join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<_>, HiveLibError>>()?
            .into_iter()
            .unzip();

        if keys.is_empty() {
            debug!("Had no keys to push, ending KeyStep early.");
            return Ok(());
        }

        let msg = key_agent::keys::Keys { keys };

        trace!("Will send message {msg:?}");

        let buf = msg.encode_to_vec();

        let mut command = get_elevated_command(
            if should_apply_locally(ctx.node.allow_local_deployment, &ctx.name.to_string()) {
                None
            } else {
                Some(&ctx.node.target)
            },
            ChildOutputMode::Raw,
            ctx.modifiers,
        )
        .await?;
        let command_string = format!("{agent_directory}/bin/key_agent {}", buf.len());

        let mut child = command.run_command(command_string, true, ctx.clobber_lock.clone())?;

        child.write_stdin(buf).await?;

        for buf in bufs {
            trace!("Pushing buf");
            child.write_stdin(buf).await?;
        }

        let status = child
            .wait_till_success()
            .await
            .map_err(HiveLibError::CommandError)?;

        debug!("status: {status:?}");

        Ok(())
    }
}

impl ExecuteStep for PushKeyAgent {
    fn should_execute(&self, ctx: &Context) -> bool {
        if ctx.no_keys {
            return false;
        }

        matches!(
            &ctx.goal,
            Goal::Keys | Goal::SwitchToConfiguration(SwitchToConfigurationGoal::Switch)
        )
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
            push(
                ctx.node,
                ctx.name,
                Push::Path(&agent_directory),
                ctx.modifiers,
                ctx.clobber_lock.clone(),
            )
            .await?;
        }

        ctx.state.key_agent_directory = Some(agent_directory);

        Ok(())
    }
}
