use nix::sys::termios::{LocalFlags, SetArg, Termios, tcgetattr, tcsetattr};
use nix::{
    poll::{PollFd, PollFlags, PollTimeout, poll},
    unistd::{pipe as posix_pipe, read as posix_read, write as posix_write},
};
use portable_pty::{NativePtySystem, PtySize};
use rand::distr::Alphabetic;
use std::collections::VecDeque;
use std::sync::mpsc::{self, Sender};
use std::sync::{Condvar, Mutex};
use std::thread::JoinHandle;
use std::{
    io::{Read, Write},
    os::fd::{AsFd, OwnedFd},
    sync::Arc,
};
use tracing::{debug, error, info, trace};

use crate::errors::DetachedError;
use crate::nix_log::NixLog;
use crate::{
    commands::{ChildOutputMode, WireCommand, WireCommandChip},
    errors::HiveLibError,
    hive::node::Target,
};

type MasterWriter = Box<dyn Write + Send>;
type MasterReader = Box<dyn Read + Send>;
type Child = Box<dyn portable_pty::Child + Send + Sync>;

pub(crate) struct ElevatedCommand<'t> {
    target: Option<&'t Target>,
    output_mode: Arc<ChildOutputMode>,
    succeed_needle: Arc<String>,
    failed_needle: Arc<String>,
    start_needle: Arc<String>,
}

pub(crate) struct ElevatedChildChip {
    child: Child,

    cancel_stdin_pipe_w: OwnedFd,
    write_stdin_pipe_w: OwnedFd,

    error_collection: Arc<Mutex<VecDeque<String>>>,

    command_string: String,

    completion_status: Arc<CompletionStatus>,
    stdout_handle: JoinHandle<Result<(), DetachedError>>,
}

struct StdinTermiosAttrGuard(Termios);

struct CompletionStatus {
    completed: Mutex<bool>,
    success: Mutex<Option<bool>>,
    condvar: Condvar,
}

struct WatchStdinArguments {
    began_tx: Sender<()>,
    reader: MasterReader,
    succeed_needle: Arc<String>,
    failed_needle: Arc<String>,
    start_needle: Arc<String>,
    output_mode: Arc<ChildOutputMode>,
    collection: Arc<Mutex<VecDeque<String>>>,
    completion_status: Arc<CompletionStatus>,
}

/// the underlying command began
const THREAD_BEGAN_SIGNAL: &[u8; 1] = b"b";
const THREAD_QUIT_SIGNAL: &[u8; 1] = b"q";

impl<'t> WireCommand<'t> for ElevatedCommand<'t> {
    type ChildChip = ElevatedChildChip;

    async fn spawn_new(
        target: Option<&'t Target>,
        output_mode: ChildOutputMode,
    ) -> Result<ElevatedCommand<'t>, HiveLibError> {
        let output_mode = Arc::new(output_mode);
        let tmp_prefix = rand::distr::SampleString::sample_string(&Alphabetic, &mut rand::rng(), 5);
        let succeed_needle = Arc::new(format!("{tmp_prefix}_WIRE_QUIT"));
        let failed_needle = Arc::new(format!("{tmp_prefix}_WIRE_FAIL"));
        let start_needle = Arc::new(format!("{tmp_prefix}_WIRE_START"));

        Ok(Self {
            target,
            output_mode,
            succeed_needle,
            failed_needle,
            start_needle,
        })
    }

    #[allow(clippy::too_many_lines)]
    fn run_command_with_env<S: AsRef<str>>(
        &mut self,
        command_string: S,
        keep_stdin_open: bool,
        envs: std::collections::HashMap<String, String>,
        clobber_lock: Arc<Mutex<()>>,
    ) -> Result<Self::ChildChip, HiveLibError> {
        if let Some(target) = self.target {
            if target.user != "root".into() {
                eprintln!(
                    "Non-root user: Please authenticate for \"sudo {}\"",
                    command_string.as_ref(),
                );
            }
        }

        let pty_system = NativePtySystem::default();
        let pty_pair = portable_pty::PtySystem::openpty(&pty_system, PtySize::default()).unwrap();

        if let Some(fd) = pty_pair.master.as_raw_fd() {
            // convert raw fd to a BorrowedFd
            // safe as `fd` is dropped well before `pty_pair.master`
            let fd = unsafe { std::os::unix::io::BorrowedFd::borrow_raw(fd) };
            let mut termios = tcgetattr(fd)
                .map_err(|x| HiveLibError::DetachedError(DetachedError::TermAttrs(x)))?;

            termios.local_flags &= !LocalFlags::ECHO;
            // Key agent does not work well without canonical mode
            termios.local_flags &= !LocalFlags::ICANON;
            // Actually quit
            termios.local_flags &= !LocalFlags::ISIG;

            tcsetattr(fd, SetArg::TCSANOW, &termios)
                .map_err(|x| HiveLibError::DetachedError(DetachedError::TermAttrs(x)))?;
        }

        let command_string = &format!(
            "echo '{start}' && {command} {flags} && echo '{succeed}' || echo '{failed}'",
            start = self.start_needle,
            succeed = self.succeed_needle,
            failed = self.failed_needle,
            command = command_string.as_ref(),
            flags = match *self.output_mode {
                ChildOutputMode::Nix => "--log-format internal-json",
                ChildOutputMode::Raw => "",
            }
        );

        debug!("{command_string}");

        let mut command = if let Some(target) = self.target {
            let mut command = create_sync_ssh_command(target)?;

            // force ssh to use our pesudo terminal
            command.arg("-tt");

            command
        } else {
            let mut command = portable_pty::CommandBuilder::new("sh");

            command.arg("-c");

            command
        };

        command.args([&format!("sudo -u root -- sh -c \"{command_string}\"")]);

        // give command all env vars
        for (key, value) in envs {
            command.env(key, value);
        }

        let clobber_guard = clobber_lock.lock().unwrap();
        let _guard = StdinTermiosAttrGuard::new().map_err(HiveLibError::DetachedError)?;
        let child = pty_pair
            .slave
            .spawn_command(command)
            .map_err(|x| HiveLibError::DetachedError(DetachedError::PortablePty(x)))?;

        // Release any handles owned by the slave: we don't need it now
        // that we've spawned the child.
        drop(pty_pair.slave);

        let reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|x| HiveLibError::DetachedError(DetachedError::PortablePty(x)))?;
        let master_writer = pty_pair
            .master
            .take_writer()
            .map_err(|x| HiveLibError::DetachedError(DetachedError::PortablePty(x)))?;

        let error_collection = Arc::new(Mutex::new(VecDeque::<String>::with_capacity(10)));
        let (began_tx, began_rx) = mpsc::channel::<()>();
        let completion_status = Arc::new(CompletionStatus::new());

        let stdout_handle = {
            let arguments = WatchStdinArguments {
                began_tx,
                reader,
                succeed_needle: self.succeed_needle.clone(),
                failed_needle: self.failed_needle.clone(),
                start_needle: self.start_needle.clone(),
                output_mode: self.output_mode.clone(),
                collection: error_collection.clone(),
                completion_status: completion_status.clone(),
            };

            std::thread::spawn(move || dynamic_watch_sudo_stdout(arguments))
        };

        let (write_stdin_pipe_r, write_stdin_pipe_w) =
            posix_pipe().map_err(|x| HiveLibError::DetachedError(DetachedError::PosixPipe(x)))?;
        let (cancel_stdin_pipe_r, cancel_stdin_pipe_w) =
            posix_pipe().map_err(|x| HiveLibError::DetachedError(DetachedError::PosixPipe(x)))?;

        std::thread::spawn(move || {
            watch_stdin_from_user(&cancel_stdin_pipe_r, master_writer, &write_stdin_pipe_r)
        });

        info!("Setup threads");

        let () = began_rx
            .recv()
            .map_err(|x| HiveLibError::DetachedError(DetachedError::RecvError(x)))?;

        drop(clobber_guard);

        if keep_stdin_open {
            trace!("Sending THREAD_BEGAN_SIGNAL");

            posix_write(&cancel_stdin_pipe_w, THREAD_BEGAN_SIGNAL)
                .map_err(|x| HiveLibError::DetachedError(DetachedError::PosixPipe(x)))?;
        } else {
            trace!("Sending THREAD_QUIT_SIGNAL");

            posix_write(&cancel_stdin_pipe_w, THREAD_QUIT_SIGNAL)
                .map_err(|x| HiveLibError::DetachedError(DetachedError::PosixPipe(x)))?;
        }

        Ok(ElevatedChildChip {
            child,
            cancel_stdin_pipe_w,
            write_stdin_pipe_w,
            error_collection,
            command_string: command_string.clone(),
            completion_status,
            stdout_handle,
        })
    }
}

impl CompletionStatus {
    fn new() -> Self {
        CompletionStatus {
            completed: Mutex::new(false),
            success: Mutex::new(None),
            condvar: Condvar::new(),
        }
    }

    fn mark_completed(&self, was_successful: bool) {
        let mut completed = self.completed.lock().unwrap();
        let mut success = self.success.lock().unwrap();

        *completed = true;
        *success = Some(was_successful);

        self.condvar.notify_all();
    }

    fn wait(&self) -> Option<bool> {
        let mut completed = self.completed.lock().unwrap();

        while !*completed {
            completed = self.condvar.wait(completed).unwrap();
        }

        *self.success.lock().unwrap()
    }
}

impl WireCommandChip for ElevatedChildChip {
    type ExitStatus = portable_pty::ExitStatus;

    async fn wait_till_success(mut self) -> Result<Self::ExitStatus, DetachedError> {
        info!("trying to grab status...");

        drop(self.write_stdin_pipe_w);

        let exit_status = tokio::task::spawn_blocking(move || self.child.wait())
            .await
            .map_err(DetachedError::JoinError)?
            .map_err(DetachedError::WaitForStatus)?;

        debug!("exit_status: {exit_status:?}");

        self.stdout_handle
            .join()
            .map_err(|_| DetachedError::ThreadPanic)??;
        let success = self.completion_status.wait();
        let _ = posix_write(&self.cancel_stdin_pipe_w, THREAD_QUIT_SIGNAL);

        if let Some(true) = success {
            return Ok(exit_status);
        }

        debug!("child did not succeed");

        let mut collection = self.error_collection.lock().unwrap();
        let logs = collection.make_contiguous().join("\n");

        Err(DetachedError::CommandFailed {
            command_ran: self.command_string,
            logs,
            code: format!("code {}", exit_status.exit_code()),
            reason: match success {
                Some(_) => "marked-unsuccessful",
                None => "child-crashed-before-succeeding",
            },
        })
    }

    async fn write_stdin(&self, data: Vec<u8>) -> Result<(), HiveLibError> {
        trace!("Writing {} bytes to stdin", data.len());

        posix_write(&self.write_stdin_pipe_w, &data)
            .map_err(|x| HiveLibError::DetachedError(DetachedError::PosixPipe(x)))?;

        Ok(())
    }
}

impl StdinTermiosAttrGuard {
    fn new() -> Result<Self, DetachedError> {
        let stdin = std::io::stdin();
        let stdin_fd = stdin.as_fd();

        let mut termios = tcgetattr(stdin_fd).map_err(DetachedError::TermAttrs)?;
        let original_termios = termios.clone();

        termios.local_flags &= !(LocalFlags::ECHO | LocalFlags::ICANON);
        tcsetattr(stdin_fd, SetArg::TCSANOW, &termios).map_err(DetachedError::TermAttrs)?;

        Ok(StdinTermiosAttrGuard(original_termios))
    }
}

impl Drop for StdinTermiosAttrGuard {
    fn drop(&mut self) {
        let stdin = std::io::stdin();
        let stdin_fd = stdin.as_fd();

        let _ = tcsetattr(stdin_fd, SetArg::TCSANOW, &self.0);
    }
}

fn create_sync_ssh_command(target: &Target) -> Result<portable_pty::CommandBuilder, HiveLibError> {
    let mut command = portable_pty::CommandBuilder::new("ssh");

    command.args(["-l", target.user.as_ref()]);
    command.arg(target.get_preffered_host()?.as_ref());
    command.args(["-p", &target.port.to_string()]);

    Ok(command)
}

fn dynamic_watch_sudo_stdout(arguments: WatchStdinArguments) -> Result<(), DetachedError> {
    let WatchStdinArguments {
        began_tx,
        mut reader,
        succeed_needle,
        failed_needle,
        start_needle,
        output_mode,
        collection,
        completion_status,
    } = arguments;

    let mut buffer = [0u8; 1024];
    let mut stdout = std::io::stdout();
    let mut began = false;

    'outer: loop {
        match reader.read(&mut buffer) {
            Ok(0) => break 'outer,
            Ok(n) => {
                let new_data = String::from_utf8_lossy(&buffer[..n]);

                for line in new_data.split_inclusive('\n') {
                    trace!("line: {line}");

                    if line.contains(start_needle.as_ref()) {
                        debug!("{start_needle} was found, switching mode...");
                        let _ = began_tx.send(());
                        began = true;
                        continue;
                    }

                    if line.contains(succeed_needle.as_ref()) {
                        debug!("{succeed_needle} was found, marking child as succeeding.");
                        completion_status.mark_completed(true);
                        break 'outer;
                    }

                    if line.contains(failed_needle.as_ref()) {
                        debug!("{failed_needle} was found, elevated child did not succeed.");
                        completion_status.mark_completed(false);
                        break 'outer;
                    }

                    if began {
                        let log = output_mode.trace(line.to_string());

                        if let Some(NixLog::Internal(log)) = log {
                            if let Some(message) = log.get_errorish_message() {
                                let mut queue = collection.lock().unwrap();
                                // add at most 10 message to the front, drop the rest.
                                queue.push_front(message);
                                queue.truncate(10);
                            }
                        }
                    } else {
                        stdout
                            .write_all(new_data.as_bytes())
                            .map_err(DetachedError::WritingClientStdout)?;
                        stdout.flush().map_err(DetachedError::WritingClientStdout)?;
                    }
                }
            }
            Err(e) => {
                eprintln!("Error reading from PTY: {e}");
                break;
            }
        }
    }

    let _ = began_tx.send(());

    // failsafe if there were errors or the reader stopped
    if !*completion_status.completed.lock().unwrap() {
        completion_status.mark_completed(false);
    }

    debug!("stdout: goodbye");

    Ok(())
}

/// Exits on any data written to `cancel_pipe_r`
fn watch_stdin_from_user(
    cancel_pipe_r: &OwnedFd,
    mut master_writer: MasterWriter,
    write_pipe_r: &OwnedFd,
) -> Result<(), DetachedError> {
    const WRITER_POSITION: usize = 0;
    const SIGNAL_POSITION: usize = 1;
    const USER_POSITION: usize = 2;

    let mut buffer = [0u8; 1024];
    let stdin = std::io::stdin();
    let mut cancel_pipe_buf = [0u8; 1];

    let user_stdin_fd = std::os::fd::AsFd::as_fd(&stdin);
    let cancel_pipe_r_fd = cancel_pipe_r.as_fd();

    let mut all_fds = vec![
        PollFd::new(write_pipe_r.as_fd(), PollFlags::POLLIN),
        PollFd::new(cancel_pipe_r.as_fd(), PollFlags::POLLIN),
        PollFd::new(user_stdin_fd, PollFlags::POLLIN),
    ];

    loop {
        match poll(&mut all_fds, PollTimeout::NONE) {
            Ok(0) => {} // timeout, impossible
            Ok(_) => {
                // The user stdin pipe can be removed
                if all_fds.get(USER_POSITION).is_some() {
                    if let Some(events) = all_fds[USER_POSITION].revents() {
                        if events.contains(PollFlags::POLLIN) {
                            trace!("Got stdin from user...");
                            let n = posix_read(user_stdin_fd, &mut buffer)
                                .map_err(DetachedError::PosixPipe)?;
                            master_writer
                                .write_all(&buffer[..n])
                                .map_err(DetachedError::WritingMasterStdout)?;
                            master_writer
                                .flush()
                                .map_err(DetachedError::WritingMasterStdout)?;
                        }
                    }
                }

                if let Some(events) = all_fds[WRITER_POSITION].revents() {
                    if events.contains(PollFlags::POLLIN) {
                        trace!("Got stdin from writer...");
                        let n = posix_read(write_pipe_r, &mut buffer)
                            .map_err(DetachedError::PosixPipe)?;
                        master_writer
                            .write_all(&buffer[..n])
                            .map_err(DetachedError::WritingMasterStdout)?;
                        master_writer
                            .flush()
                            .map_err(DetachedError::WritingMasterStdout)?;
                    }
                }

                if let Some(events) = all_fds[SIGNAL_POSITION].revents() {
                    if events.contains(PollFlags::POLLIN) {
                        let n = posix_read(cancel_pipe_r_fd, &mut cancel_pipe_buf)
                            .map_err(DetachedError::PosixPipe)?;
                        let message = &cancel_pipe_buf[..n];

                        trace!("Got byte from signal pipe: {message:?}");

                        if message == THREAD_QUIT_SIGNAL {
                            return Ok(());
                        }

                        if message == THREAD_BEGAN_SIGNAL {
                            all_fds.remove(USER_POSITION);
                        }
                    }
                }
            }
            Err(e) => {
                error!("Poll error: {e}");
                break;
            }
        }
    }

    debug!("stdin_thread: goodbye");
    Ok(())
}
