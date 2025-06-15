#![feature(let_chains)]
#![deny(clippy::pedantic)]
#![allow(clippy::must_use_candidate)]
use hive::{
    node::{Name, SwitchToConfigurationGoal, Target},
    steps::keys::Error,
};
use nix_log::{NixLog, Trace};
use std::path::PathBuf;
use thiserror::Error;
use tokio::{process::Command, task::JoinError};

pub mod hive;
mod nix;
mod nix_log;
mod test_macros;

fn create_ssh_command(target: &Target, sudo: bool) -> Command {
    let mut command = Command::new("ssh");

    command
        .args(["-l", target.user.as_ref()])
        .arg(target.host.as_ref())
        .args(["-p", &target.port.to_string()]);

    if sudo && target.user != "root".into() {
        command.args(["sudo", "-H", "--"]);
    }

    command
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

    #[error("failed to evaluate nix expression (last 20 lines):\n{}", format_error_lines(.0))]
    NixEvalError(Vec<String>),

    #[error("failed to evaluate node {0} (filtered logs, run with -vvv to see all):\n{}", .1.iter().filter(|l| l.is_error()).map(std::string::ToString::to_string).collect::<Vec<String>>().join("\n"))]
    NixEvalInteralError(Name, Vec<NixLog>),

    #[error("failed to copy drv to node {0} (filtered logs, run with -vvv to see all):\n{}", .1.iter().filter(|l| l.is_error()).map(std::string::ToString::to_string).collect::<Vec<String>>().join("\n"))]
    NixCopyError(Name, Vec<NixLog>),

    #[error("failed to build node {0} (last 20 lines):\n{}", format_error_lines(.1))]
    NixBuildError(Name, Vec<String>),

    #[error("failed to run switch-to-configuration {0} on node {1} (last 20 lines):\n{}", format_error_lines(.2))]
    SwitchToConfigurationError(SwitchToConfigurationGoal, Name, Vec<String>),

    #[error("failed to run nix-env on node {0} (last 20 lines):\n{}", format_error_lines(.1))]
    NixEnvError(Name, Vec<String>),

    #[error("failed to push keys to {0} (last 20 lines):\n{}", format_error_lines(.1))]
    KeyCommandError(Name, Vec<String>),

    #[error("failed to push a key")]
    KeyError(#[source] Error),

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

    #[error("an operation failed in regards to buffers")]
    BufferOperationError(#[source] tokio::io::Error),

    #[error("failed to elevate")]
    FailedToElevate(#[source] std::io::Error),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SubCommandModifiers {
    pub show_trace: bool,
}
