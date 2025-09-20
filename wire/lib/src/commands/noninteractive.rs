use std::{
    collections::{HashMap, VecDeque},
    process::ExitStatus,
    sync::Arc,
};

use itertools::Itertools;
use tokio::{
    io::{AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::Mutex,
    task::JoinSet,
};
use tracing::{debug, trace};

use crate::{
    SubCommandModifiers,
    commands::{ChildOutputMode, WireCommand, WireCommandChip},
    errors::{CommandError, HiveLibError},
    hive::node::Target,
    nix_log::NixLog,
};

pub(crate) struct NonInteractiveCommand<'t> {
    target: Option<&'t Target>,
    output_mode: Arc<ChildOutputMode>,
    modifiers: SubCommandModifiers,
}

pub(crate) struct NonInteractiveChildChip {
    error_collection: Arc<Mutex<VecDeque<String>>>,
    stdout_collection: Arc<Mutex<VecDeque<String>>>,
    child: Child,
    joinset: JoinSet<()>,
    command_string: String,
    stdin: ChildStdin,
}

impl<'t> WireCommand<'t> for NonInteractiveCommand<'t> {
    type ChildChip = NonInteractiveChildChip;

    /// If target is Some, then the command will be ran remotely.
    /// Otherwise, the command is ran locally.
    async fn spawn_new(
        target: Option<&'t Target>,
        output_mode: ChildOutputMode,
        modifiers: SubCommandModifiers,
    ) -> Result<Self, crate::errors::HiveLibError> {
        let output_mode = Arc::new(output_mode);

        Ok(Self {
            target,
            output_mode,
            modifiers,
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
            create_sync_ssh_command(target, self.modifiers)?
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
        command.stdin(std::process::Stdio::piped());
        command.stderr(std::process::Stdio::piped());
        command.stdout(std::process::Stdio::piped());
        command.kill_on_drop(true);
        // command.env_clear();
        command.envs(envs);

        let mut child = command.spawn().unwrap();
        let error_collection = Arc::new(Mutex::new(VecDeque::<String>::with_capacity(10)));
        let stdout_collection = Arc::new(Mutex::new(VecDeque::<String>::with_capacity(10)));
        let stdin = child.stdin.take().unwrap();

        let stdout_handle = child
            .stdout
            .take()
            .ok_or(HiveLibError::CommandError(CommandError::NoHandle))?;
        let stderr_handle = child
            .stderr
            .take()
            .ok_or(HiveLibError::CommandError(CommandError::NoHandle))?;

        let mut joinset = JoinSet::new();

        joinset.spawn(handle_io(
            stderr_handle,
            self.output_mode.clone(),
            error_collection.clone(),
            true,
        ));
        joinset.spawn(handle_io(
            stdout_handle,
            self.output_mode.clone(),
            stdout_collection.clone(),
            false,
        ));

        Ok(NonInteractiveChildChip {
            error_collection,
            stdout_collection,
            child,
            joinset,
            command_string,
            stdin,
        })
    }
}

impl WireCommandChip for NonInteractiveChildChip {
    type ExitStatus = (ExitStatus, String);

    async fn wait_till_success(mut self) -> Result<Self::ExitStatus, CommandError> {
        let status = self.child.wait().await.unwrap();
        let _ = self.joinset.join_all().await;

        if !status.success() {
            let logs = self
                .error_collection
                .lock()
                .await
                .make_contiguous()
                .join("\n");

            return Err(CommandError::CommandFailed {
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

    async fn write_stdin(&mut self, data: Vec<u8>) -> Result<(), HiveLibError> {
        trace!("Writing {} bytes", data.len());
        self.stdin.write_all(&data).await.unwrap();
        Ok(())
    }
}

pub async fn handle_io<R>(
    reader: R,
    output_mode: Arc<ChildOutputMode>,
    collection: Arc<Mutex<VecDeque<String>>>,
    is_error: bool,
) where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut io_reader = tokio::io::AsyncBufReadExt::lines(BufReader::new(reader));

    while let Some(line) = io_reader.next_line().await.unwrap() {
        let log = output_mode.trace(line.clone(), is_error);

        if !is_error {
            let mut queue = collection.lock().await;
            queue.push_front(line);
        } else if let Some(NixLog::Internal(log)) = log {
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

fn create_sync_ssh_command(
    target: &Target,
    modifiers: SubCommandModifiers,
) -> Result<Command, HiveLibError> {
    let mut command = Command::new("ssh");
    command.args(target.create_ssh_args(modifiers)?);
    Ok(command)
}
