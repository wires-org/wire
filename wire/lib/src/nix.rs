use regex::Regex;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::sync::LazyLock;
use tokio::io::BufReader;
use tokio::io::{AsyncBufReadExt, AsyncRead};
use tracing::{Instrument, Span, error, info, trace};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use crate::errors::{HiveInitializationError, NixChildError};
use crate::hive::find_hive;
use crate::hive::node::Name;
use crate::nix_log::{Action, Internal, NixLog, Trace};
use crate::{HiveLibError, SubCommandModifiers};

static DIGEST_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[0-9a-z]{32}").unwrap());

pub enum EvalGoal<'a> {
    Inspect,
    GetTopLevel(&'a Name),
}

fn check_nix_available() -> bool {
    match Command::new("nix")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => true,
        Err(e) => {
            if let std::io::ErrorKind::NotFound = e.kind() {
                false
            } else {
                error!(
                    "Something weird happened checking for nix availability, {}",
                    e
                );
                false
            }
        }
    }
}

pub fn get_eval_command(
    path: &Path,
    goal: &EvalGoal,
    modifiers: SubCommandModifiers,
) -> Result<tokio::process::Command, HiveLibError> {
    assert!(check_nix_available(), "nix is not available on this system");

    let canon_path =
        find_hive(&path.canonicalize().unwrap()).ok_or(HiveLibError::HiveInitializationError(
            HiveInitializationError::NoHiveFound(path.to_path_buf()),
        ))?;

    let mut command = tokio::process::Command::new("nix");
    command.args(["--extra-experimental-features", "nix-command"]);
    command.args(["--extra-experimental-features", "flakes"]);
    command.args(["eval", "--json"]);

    if modifiers.show_trace {
        command.arg("--show-trace");
    }

    if canon_path.ends_with("flake.nix") {
        command.arg(format!("{}#wire", canon_path.to_str().unwrap()));
        command.arg("--apply");

        command.arg(format!(
            "hive: {goal}",
            goal = match goal {
                EvalGoal::Inspect => "hive.inspect".to_string(),
                EvalGoal::GetTopLevel(node) => format!("hive.topLevels.{node}"),
            }
        ));
    } else {
        command.args(["--file", &canon_path.to_string_lossy()]);

        command.arg(match goal {
            EvalGoal::Inspect => "inspect".to_string(),
            EvalGoal::GetTopLevel(node) => format!("topLevels.{node}"),
        });
    }

    Ok(command)
}

pub async fn handle_io<R>(reader: R, should_trace: bool) -> Result<Vec<NixLog>, HiveLibError>
where
    R: AsyncRead + Unpin,
{
    let mut io_reader = BufReader::new(reader).lines();
    let mut collect = Vec::new();

    while let Some(line) = io_reader
        .next_line()
        .await
        .map_err(|err| HiveLibError::NixChildError(NixChildError::SpawnFailed(err)))?
    {
        let log = serde_json::from_str::<Internal>(line.strip_prefix("@nix ").unwrap_or(&line))
            .map(NixLog::Internal)
            .unwrap_or(NixLog::Raw(line.to_string()));

        // Throw out stop logs
        if let NixLog::Internal(Internal {
            action: Action::Stop,
        }) = log
        {
            continue;
        }

        if cfg!(debug_assertions) {
            trace!(line);
        }

        if should_trace {
            match log {
                NixLog::Raw(ref string) => info!("{string}"),
                NixLog::Internal(ref internal) => internal.trace(),
            }

            Span::current().pb_set_message(&DIGEST_RE.replace_all(&log.to_string(), "…"));
        }

        collect.push(log);
    }

    Ok(collect)
}

pub trait StreamTracing {
    async fn execute(
        &mut self,
        log_stderr: bool,
    ) -> Result<(ExitStatus, Vec<NixLog>, Vec<NixLog>), HiveLibError>;
}

impl StreamTracing for tokio::process::Command {
    async fn execute(
        &mut self,
        log_stderr: bool,
    ) -> Result<(ExitStatus, Vec<NixLog>, Vec<NixLog>), HiveLibError> {
        let mut child = self
            .args(["--log-format", "internal-json"])
            .stderr(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|err| HiveLibError::NixChildError(NixChildError::SpawnFailed(err)))?;

        let stdout_handle = child
            .stdout
            .take()
            .ok_or(HiveLibError::NixChildError(NixChildError::NoHandle))?;
        let stderr_handle = child
            .stderr
            .take()
            .ok_or(HiveLibError::NixChildError(NixChildError::NoHandle))?;

        let stderr_task = tokio::spawn(handle_io(stderr_handle, log_stderr).in_current_span());
        let stdout_task = tokio::spawn(handle_io(stdout_handle, false));

        let handle = tokio::spawn(async move {
            child
                .wait()
                .await
                .map_err(|err| HiveLibError::NixChildError(NixChildError::SpawnFailed(err)))
        });

        let (result, stdout, stderr) = tokio::try_join!(handle, stdout_task, stderr_task)
            .map_err(|err| HiveLibError::NixChildError(NixChildError::JoinError(err)))?;

        Ok((result?, stdout?, stderr?))
    }
}
