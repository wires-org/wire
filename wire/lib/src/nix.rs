use regex::Regex;
use std::env;
use std::path::Path;
use std::process::{Command, ExitStatus};
use std::sync::LazyLock;
use tokio::io::BufReader;
use tokio::io::{AsyncBufReadExt, AsyncRead};
use tracing::{Instrument, Span, error, info, trace};
use tracing_indicatif::span_ext::IndicatifSpanExt;

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
) -> tokio::process::Command {
    let runtime = match env::var_os("WIRE_RUNTIME") {
        Some(runtime) => runtime.into_string().unwrap(),
        None => panic!("WIRE_RUNTIME environment variable not set"),
    };

    assert!(check_nix_available(), "nix is not available on this system");

    let canon_path = path.canonicalize().unwrap();

    let mut command = tokio::process::Command::new("nix");
    command.args(["--extra-experimental-features", "nix-command"]);
    command.args(["--extra-experimental-features", "flakes"]);
    command.args(["eval", "--json", "--impure"]);
    if modifiers.show_trace {
        command.arg("--show-trace");
    }
    command.args(["--expr"]);

    command.arg(format!(
        "let flake = {flake}; evaluate = import {runtime}/evaluate.nix; hive = evaluate {{hive = \
         {hive}; path = {path}; nixosConfigurations = {nixosConfigurations}; nixpkgs = \
         {nixpkgs};}}; in {goal}",
        flake = if canon_path.ends_with("flake.nix") {
            format!(
                "(builtins.getFlake \"git+file://{path}\")",
                path = canon_path.parent().unwrap().to_str().unwrap(),
            )
        } else {
            "null".to_string()
        },
        hive = if canon_path.ends_with("flake.nix") {
            "flake.colmena".to_string()
        } else {
            format!("import {path}", path = canon_path.to_str().unwrap())
        },
        nixosConfigurations = if canon_path.ends_with("flake.nix") {
            "flake.nixosConfigurations or {}".to_string()
        } else {
            "{}".to_string()
        },
        nixpkgs = if canon_path.ends_with("flake.nix") {
            "flake.inputs.nixpkgs.outPath or null".to_string()
        } else {
            "null".to_string()
        },
        path = canon_path.to_str().unwrap(),
        goal = match goal {
            EvalGoal::Inspect => "hive.inspect".to_string(),
            EvalGoal::GetTopLevel(node) => format!("hive.getTopLevel \"{node}\""),
        }
    ));

    command
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
        .map_err(HiveLibError::SpawnFailed)?
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
            .map_err(HiveLibError::SpawnFailed)?;

        let stdout_handle = child.stdout.take().ok_or(HiveLibError::NoHandle)?;
        let stderr_handle = child.stderr.take().ok_or(HiveLibError::NoHandle)?;

        let stderr_task = tokio::spawn(handle_io(stderr_handle, log_stderr).in_current_span());
        let stdout_task = tokio::spawn(handle_io(stdout_handle, false));

        let handle =
            tokio::spawn(async move { child.wait().await.map_err(HiveLibError::SpawnFailed) });

        let (result, stdout, stderr) =
            tokio::try_join!(handle, stdout_task, stderr_task).map_err(HiveLibError::JoinError)?;

        Ok((result?, stdout?, stderr?))
    }
}
