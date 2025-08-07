#![deny(clippy::pedantic)]
#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]
use hive::{
    node::{Name, SwitchToConfigurationGoal, Target},
    steps::keys::KeyError,
};
use miette::Diagnostic;
use nix_log::{NixLog, Trace};
use std::path::PathBuf;
use thiserror::Error;
use tokio::{process::Command, task::JoinError};

pub mod hive;
mod nix;
mod nix_log;

#[cfg(test)]
mod test_macros;

#[cfg(test)]
mod test_support;

fn create_ssh_command(target: &Target, sudo: bool) -> Result<Command, HiveLibError> {
    let mut command = Command::new("ssh");

    command
        .args(["-l", target.user.as_ref()])
        .arg(target.get_preffered_host()?.as_ref())
        .args(["-p", &target.port.to_string()]);

    if sudo && target.user != "root".into() {
        command.args(["sudo", "-H", "--"]);
    }

    Ok(command)
}

fn format_error_lines(lines: &[String]) -> String {
    lines
        .iter()
        .rev()
        .take(20)
        .rev()
        .cloned()
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Debug, Diagnostic, Error)]
pub enum HiveInitializationError {
    #[diagnostic(
        code(wire::HiveInit::NoHiveFound),
        help(
            "Double check the path is correct. You can adjust the hive path with `--path` when the hive lies outside of the CWD."
        )
    )]
    #[error("No hive could be found in {}", .0.display())]
    NoHiveFound(PathBuf),

    #[diagnostic(
        code(wire::HiveInit::NixEvalError),
        help("Check your hive is syntactically valid.")
    )]
    #[error("failed to evaluate your hive! last 20 lines:\n{}", format_error_lines(.0))]
    NixEvalError(Vec<String>),
}

#[derive(Debug, Diagnostic, Error)]
pub enum NetworkError {
    #[diagnostic(
        code(wire::Network::HostUnreachable),
        help(
            "If you failed due to a fault in DNS, note that a node can have multiple targets defined."
        )
    )]
    #[error("Cannot reach host {0}")]
    HostUnreachable(String),

    #[diagnostic(code(wire::Network::HostUnreachableAfterReboot))]
    #[error("Cannot reach host {0} after reboot")]
    HostUnreachableAfterReboot(String),

    #[diagnostic(code(wire::Network::HostsExhausted))]
    #[error("Ran out of contactable hosts")]
    HostsExhausted,
}

#[derive(Debug, Diagnostic, Error)]
pub enum HiveLibError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    HiveInitializationError(HiveInitializationError),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NetworkError(NetworkError),

    #[error("failed to execute nix command")]
    NixExecError(#[source] tokio::io::Error),

    #[error(
        "failed to evaluate node {0} (filtered logs, run with -vvv to see all):\n{log}",
        log = .1.iter().filter(|l| l.is_error()).map(std::string::ToString::to_string).collect::<Vec<String>>().join("\n"))
    ]
    NixEvalInteralError(Name, Vec<NixLog>),

    #[error(
        "failed to copy drv to node {0} (filtered logs, run with -vvv to see all):\n{log}", 
        log = .1.iter().filter(|l| l.is_error()).map(std::string::ToString::to_string).collect::<Vec<String>>().join("\n"))
    ]
    NixCopyError(Name, Vec<NixLog>),

    #[error("failed to build node {0} (last 20 lines):\n{lines}", lines = format_error_lines(.1))]
    NixBuildError(Name, Vec<String>),

    #[error("failed to run switch-to-configuration {0} on node {1} (last 20 lines):\n{lines}", lines = format_error_lines(.2))]
    SwitchToConfigurationError(SwitchToConfigurationGoal, Name, Vec<String>),

    #[error("failed to run nix-env on node {0} (last 20 lines):\n{lines}", lines = format_error_lines(.1))]
    NixEnvError(Name, Vec<String>),

    #[error("failed to push keys to {0} (last 20 lines):\n{lines}", lines = format_error_lines(.1))]
    KeyCommandError(Name, Vec<String>),

    #[error("failed to push a key")]
    KeyError(#[source] KeyError),

    #[error("node {0} not exist in hive")]
    NodeDoesNotExist(String),

    #[error("failed to execute command")]
    SpawnFailed(#[source] tokio::io::Error),

    #[error("failed to join task")]
    JoinError(#[source] JoinError),

    #[error("there was no handle to io on the child process")]
    NoHandle,

    #[error("failed to parse nix log \"{0}\"")]
    ParseLogError(String, #[source] serde_json::Error),

    #[error("failed to parse internal wire json. please create an issue!")]
    ParseEvaluateError(#[source] serde_json::Error),

    #[error("an operation failed in regards to buffers")]
    BufferOperationError(#[source] tokio::io::Error),

    #[error("failed to elevate")]
    FailedToElevate(#[source] std::io::Error),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SubCommandModifiers {
    pub show_trace: bool,
}
