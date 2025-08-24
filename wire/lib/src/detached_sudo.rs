use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use nix::unistd::{pipe, read, write};
use std::os::fd::AsFd;

use portable_pty::NativePtySystem;
use portable_pty::PtySize;
use portable_pty::PtySystem;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::{io::Read, process::Stdio};
use tokio_util::sync::CancellationToken;
use tracing::debug;

use rand::distr::{Alphabetic, SampleString};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::{Child, ChildStdout, Command},
    select,
    task::JoinHandle,
};
use tracing::{info, warn};

use crate::{
    errors::{DetachedError, HiveLibError},
    hive::node::Target,
};

const PREFIX_LENGTH: usize = 10;

pub(crate) struct DetachedCommand<'a> {
    tmp_path_base: String,

    cancel_token: CancellationToken,
    cancel_needle: Arc<String>,

    stdout: Child,
    stderr: Child,
    stdout_task: JoinHandle<Result<(), DetachedError>>,
    stderr_task: JoinHandle<Result<(), DetachedError>>,

    target: &'a Target,
}

impl<'a> DetachedCommand<'a> {
    pub async fn handle_io<R>(
        reader: R,
        cancel_needle: Arc<String>,
        cancel: CancellationToken,
    ) -> Result<(), DetachedError>
    where
        R: AsyncRead + Unpin,
    {
        let mut io_reader = BufReader::new(reader).lines();

        debug!("spawning handle_io");

        loop {
            select! {
                line = io_reader.next_line() => {
                    if let Some(line) = line.unwrap() {
                        info!("{line}");

                        if line.contains(&*cancel_needle) {
                            debug!("{cancel_needle} was found, cancelling reader threads");
                            cancel.cancel();
                            break;
                        }
                    } else {
                        // debug!("Line was none...");
                    }
                },
                () = cancel.cancelled() => {
                    debug!("Reader thread cancelled.");
                    return Ok(());
                }
            }
        }

        // Tear down sister threads
        debug!("Cancelling from inside handle_io");
        cancel.cancel();

        Ok(())
    }

    fn create_ssh_command(target: &Target) -> Result<Command, HiveLibError> {
        let mut command = Command::new("ssh");

        command
            .args(["-l", target.user.as_ref()])
            .arg(target.get_preffered_host()?.as_ref())
            .args(["-p", &target.port.to_string()]);

        Ok(command)
    }

    fn create_sync_ssh_command(
        target: &Target,
    ) -> Result<portable_pty::CommandBuilder, HiveLibError> {
        let mut command = portable_pty::CommandBuilder::new("ssh");

        command.args(["-l", target.user.as_ref()]);
        command.arg(target.get_preffered_host()?.as_ref());
        command.args(["-p", &target.port.to_string()]);

        Ok(command)
    }

    async fn run_mkfifo(target: &Target, tmp_path_base: &str) -> Result<(), HiveLibError> {
        info!("{tmp_path_base}.{{out,err,in}}");
        let directory = PathBuf::from(tmp_path_base);
        info!(
            "mkdir \"{}\" && mkfifo -m 666 {tmp_path_base}.out {tmp_path_base}.err {tmp_path_base}.in",
            directory.parent().unwrap().display()
        );
        let create = DetachedCommand::create_ssh_command(target)?
            .args([&format!(
                "mkdir \"{}\"; mkfifo -m 666 {tmp_path_base}.{{out,err,in}}",
                directory.parent().unwrap().display()
            )])
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .output()
            .await
            .map_err(|err| HiveLibError::DetachedError(DetachedError::SpawnMkFifo(err)))?;

        if create.status.success() {
            return Ok(());
        }

        Err(HiveLibError::DetachedError(DetachedError::FailToMkFifo {
            stdout: String::from_utf8_lossy(&create.stdout).to_string(),
            stderr: String::from_utf8_lossy(&create.stderr).to_string(),
        }))
    }

    fn create_reader(
        target: &Target,
        cancel_needle: &Arc<String>,
        path: &str,
    ) -> Result<(Child, ChildStdout), HiveLibError> {
        let mut reader = DetachedCommand::create_ssh_command(target)?
            .args([&format!(
                "echo BEGIN && tail -f {path} && echo {cancel_needle}"
            )])
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|err| HiveLibError::DetachedError(DetachedError::JoinError(err)))?;

        let reader_handle = reader
            .stdout
            .take()
            .ok_or(HiveLibError::DetachedError(DetachedError::NoHandle))?;

        Ok((reader, reader_handle))
    }

    pub(crate) async fn spawn_new(target: &'a Target) -> Result<Self, HiveLibError> {
        info!("{target:?}");

        // let tmp_prefix = Alphabetic.sample_string(&mut rand::rng(), PREFIX_LENGTH);
        let tmp_prefix = "const";
        let tmp_path_base = format!("/tmp/wire/{tmp_prefix}_wire_detatched");
        let cancel_token = CancellationToken::new();
        let cancel_needle = Arc::new(format!("{tmp_prefix}_WIRE_QUIT"));

        DetachedCommand::run_mkfifo(target, &tmp_path_base).await?;

        let (stdout, stdout_handle) = DetachedCommand::create_reader(
            target,
            &cancel_needle,
            &format!("{tmp_path_base}.out"),
        )?;
        let (stderr, stderr_handle) = DetachedCommand::create_reader(
            target,
            &cancel_needle,
            &format!("{tmp_path_base}.err"),
        )?;

        let stdout_task = tokio::spawn(DetachedCommand::handle_io(
            stdout_handle,
            cancel_needle.clone(),
            cancel_token.clone(),
        ));
        let stderr_task = tokio::spawn(DetachedCommand::handle_io(
            stderr_handle,
            cancel_needle.clone(),
            cancel_token.clone(),
        ));

        Ok(Self {
            tmp_path_base,
            cancel_token,
            cancel_needle,
            stdout,
            stderr,
            stdout_task,
            stderr_task,
            target,
        })
    }

    #[allow(clippy::similar_names)]
    #[allow(clippy::too_many_lines)]
    pub(crate) fn run_sudo_command<S: AsRef<str>>(
        &self,
        command_string: S,
    ) -> Result<Box<dyn portable_pty::Child + Send + Sync>, HiveLibError> {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();

        let string_format = &format!(
            "echo 'WIRE_BEGIN' && {command} > {base_path}.out 2> {base_path}.err < /dev/null; echo {needle} > {base_path}.out",
            command = command_string.as_ref(),
            base_path = self.tmp_path_base,
            needle = self.cancel_needle
        );

        debug!("{string_format}");

        let mut command = DetachedCommand::create_sync_ssh_command(self.target)?;

        command.args([
            // force ssh to use our terminal
            "-tt",
            &format!("sudo -u root sh -c \"{string_format}\""),
        ]);

        warn!(
            "Please authenticate for \"sudo {}\"",
            command_string.as_ref()
        );

        let child = pair.slave.spawn_command(command).unwrap();

        // Release any handles owned by the slave: we don't need it now
        // that we've spawned the child.
        drop(pair.slave);

        // let (tx, rx) = std::sync::mpsc::channel();

        let (pipe_r, pipe_w) = pipe().unwrap();

        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut master_writer = pair.master.take_writer().unwrap();
        let cancel_token = CancellationToken::new();

        let thread_token = cancel_token.clone();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            let mut stdout = std::io::stdout();
            loop {
                if thread_token.is_cancelled() {
                    break;
                }

                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        let output = String::from_utf8_lossy(&buffer[..n]);

                        if output.contains("WIRE_BEGIN") {
                            info!("WIRE_BEGIN was found, cancelling...");
                            thread_token.cancel();
                            break;
                        }

                        stdout.write_all(&buffer[..n]).unwrap();
                        stdout.flush().unwrap();
                    }
                    Err(e) => {
                        eprintln!("Error reading from PTY: {e}");
                        break;
                    }
                }
            }
        });

        let stdin_thread = std::thread::spawn(move || {
            let mut buffer = [0u8; 1024];
            let stdin = std::io::stdin();
            let mut pipe_buf = [0u8; 1];

            let stdin_fd = std::os::fd::AsFd::as_fd(&stdin);
            let pipe_r_fd = pipe_r.as_fd();
            // let pipe_r_fd_raw = pipe_r_fd.as_raw_fd();
            // let stdin_fd_raw = stdin_fd.as_raw_fd();

            let mut poll_fds = [
                PollFd::new(stdin_fd, PollFlags::POLLIN),
                PollFd::new(pipe_r.as_fd(), PollFlags::POLLIN),
            ];

            loop {
                match poll(&mut poll_fds, PollTimeout::NONE) {
                    Ok(0) => {} // timeout
                    Ok(_) => {
                        if let Some(events) = poll_fds[0].revents() {
                            if events.contains(PollFlags::POLLIN) {
                                debug!("Got stdin...");
                                let n = read(stdin_fd, &mut buffer).unwrap();
                                master_writer.write_all(&buffer[..n]).unwrap();
                            }
                        }
                        if let Some(events) = poll_fds[1].revents() {
                            if events.contains(PollFlags::POLLIN) {
                                debug!("Got pipe cancel...");
                                let _ = read(pipe_r_fd, &mut pipe_buf);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Poll error: {e}");
                        break;
                    }
                }
            }

            info!("stdin_thread: goodbye");
        });

        info!("Setup threads");

        loop {
            if cancel_token.is_cancelled() {
                break;
            }
        }

        info!("Cancelled...");

        write(&pipe_w, b"x").unwrap();

        info!("Joined tx");
        stdin_thread.join().unwrap();
        info!("Joined stdin");

        info!("finished raising sudo, good luck...");

        Ok(child)
    }

    fn handle_input_stream(
        rx: &std::sync::mpsc::Receiver<&[u8]>,
        cancel_token: &CancellationToken,
        mut writer: Box<dyn Write + Send>,
    ) {
        loop {
            if cancel_token.is_cancelled() {
                info!("Cancelling handle_input_stream");
                break;
            }

            if let Ok(input) = rx.try_recv() {
                if writer.write_all(input).is_err() {
                    eprintln!("Error writing to PTY");
                    break;
                }
            }
        }
    }

    pub(crate) async fn wait_until_complete(
        self,
        mut command_child: Box<dyn portable_pty::Child + Send + Sync>,
    ) -> Result<(), HiveLibError> {
        let (stdout, stderr) = futures::try_join!(self.stdout_task, self.stderr_task)
            .map_err(|err| HiveLibError::DetachedError(DetachedError::JoiningFifos(err)))?;

        let mut errors = Vec::new();
        if let Err(stdout) = stdout {
            errors.push(stdout);
        }
        if let Err(stderr) = stderr {
            errors.push(stderr);
        }

        info!("waiting for child to quit...");

        let status = command_child.wait().unwrap();

        info!("whoami exit status: {status}");

        if errors.is_empty() {
            return Ok(());
        }

        Err(HiveLibError::MultipleDetachedErrors { errors })
    }
}
