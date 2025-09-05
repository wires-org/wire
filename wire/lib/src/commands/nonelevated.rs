use std::{
    collections::{HashMap, VecDeque},
    process::{ExitStatus, Stdio},
    sync::Arc,
};

use itertools::Itertools;
use tokio::{
    io::BufReader,
    process::{Child, Command},
    sync::Mutex,
    task::JoinSet,
};
use tracing::debug;

use crate::{
    Target,
    commands::{ChildOutputMode, WireCommand, WireCommandChip},
    errors::{DetachedError, HiveLibError},
    nix_log::NixLog,
};

pub(crate) struct NonElevatedCommand<'t> {
    target: Option<&'t Target>,
    output_mode: Arc<ChildOutputMode>,
}

pub(crate) struct NonElevatedChildChip {
    error_collection: Arc<Mutex<VecDeque<String>>>,
    stdout_collection: Arc<Mutex<VecDeque<String>>>,
    child: Child,
    joinset: JoinSet<()>,
    command_string: String,
}

impl<'t> WireCommand<'t> for NonElevatedCommand<'t> {
    type ChildChip = NonElevatedChildChip;

    /// If target is Some, then the command will be ran remotely.
    /// Otherwise, the command is ran locally.
    async fn spawn_new(
        target: Option<&'t Target>,
        output_mode: ChildOutputMode,
    ) -> Result<Self, crate::errors::HiveLibError> {
        let output_mode = Arc::new(output_mode);

        Ok(Self {
            target,
            output_mode,
        })
    }

    fn run_command_with_env<S: AsRef<str>>(
        &mut self,
        command_string: S,
        _keep_stdin_open: bool,
        envs: HashMap<String, String>,
        _clobber_lock: Arc<std::sync::Mutex<()>>,
    ) -> Result<Self::ChildChip, HiveLibError> {
        let mut command = if let Some(target) = self.target {
            create_sync_ssh_command(target)?
        } else {
            let mut command = Command::new("sh");

            command.arg("-c");

            command
        };

        let command_string = format!(
            "{command_string}{extra}",
            command_string = command_string.as_ref(),
            extra = match *self.output_mode {
                ChildOutputMode::Raw => "",
                ChildOutputMode::Nix => " --log-format internal-json",
            }
        );

        command.arg(&command_string);
        command.stdin(Stdio::null());
        command.stderr(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.kill_on_drop(true);
        // command.env_clear();
        command.envs(envs);

        let mut child = command.spawn().unwrap();
        let error_collection = Arc::new(Mutex::new(VecDeque::<String>::with_capacity(10)));
        let stdout_collection = Arc::new(Mutex::new(VecDeque::<String>::with_capacity(10)));

        let stdout_handle = child
            .stdout
            .take()
            .ok_or(HiveLibError::DetachedError(DetachedError::NoHandle))?;
        let stderr_handle = child
            .stderr
            .take()
            .ok_or(HiveLibError::DetachedError(DetachedError::NoHandle))?;

        let mut joinset = JoinSet::new();

        joinset.spawn(handle_io(
            stderr_handle,
            self.output_mode.clone(),
            error_collection.clone(),
            false,
        ));
        joinset.spawn(handle_io(
            stdout_handle,
            self.output_mode.clone(),
            stdout_collection.clone(),
            true,
        ));

        Ok(NonElevatedChildChip {
            error_collection,
            stdout_collection,
            child,
            joinset,
            command_string,
        })
    }
}

impl WireCommandChip for NonElevatedChildChip {
    type ExitStatus = (ExitStatus, String);

    async fn wait_till_success(mut self) -> Result<Self::ExitStatus, DetachedError> {
        let status = self.child.wait().await.unwrap();
        let _ = self.joinset.join_all().await;

        if !status.success() {
            let logs = self
                .error_collection
                .lock()
                .await
                .make_contiguous()
                .join("\n");

            return Err(DetachedError::CommandFailed {
                command_ran: self.command_string,
                logs,
                code: match status.code() {
                    Some(code) => format!("code {code}"),
                    None => "no exit code".to_string(),
                },
                reason: "known-status",
            });
        }

        let stdout = self.stdout_collection.lock().await.iter().join("\n");

        Ok((status, stdout))
    }

    /// Unimplemented until needed.
    async fn write_stdin(&self, _data: Vec<u8>) -> Result<(), HiveLibError> {
        Ok(())
    }
}

pub async fn handle_io<R>(
    reader: R,
    output_mode: Arc<ChildOutputMode>,
    collection: Arc<Mutex<VecDeque<String>>>,
    always_collect: bool,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut io_reader = tokio::io::AsyncBufReadExt::lines(BufReader::new(reader));

    while let Some(line) = io_reader.next_line().await.unwrap() {
        if always_collect {
            let mut queue = collection.lock().await;
            queue.push_front(line);
            continue;
        }

        let log = output_mode.trace(line.clone());

        if let Some(NixLog::Internal(log)) = log {
            if let Some(message) = log.get_errorish_message() {
                let mut queue = collection.lock().await;
                queue.push_front(message);
                // add at most 10 message to the front, drop the rest.
                queue.truncate(10);
            }
        }
    }

    debug!("io_handler: goodbye!");
}

fn create_sync_ssh_command(target: &Target) -> Result<Command, HiveLibError> {
    let mut command = Command::new("ssh");

    command.args(["-l", target.user.as_ref()]);
    command.arg(target.get_preffered_host()?.as_ref());
    command.args(["-p", &target.port.to_string()]);

    Ok(command)
}
