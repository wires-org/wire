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

#[derive(Debug, Error)]
pub enum HiveLibError {
    #[error("no hive could be found in {}", .0.display())]
    NoHiveFound(PathBuf),

    #[error("failed to execute nix command")]
    NixExecError(#[source] tokio::io::Error),

    #[error("failed to evaluate your hive! is it valid? (last 20 lines):\n{}", format_error_lines(.0))]
    NixEvalError(Vec<String>),

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

    #[error("Cannot reach host {0}")]
    HostUnreachable(String),

    #[error("Cannot reach host {0} after reboot")]
    HostUnreachableAfterReboot(String),

    #[error("Ran out of contactable hosts")]
    HostsExhausted,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SubCommandModifiers {
    pub show_trace: bool,
}
