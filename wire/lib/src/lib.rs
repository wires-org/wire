#![deny(clippy::pedantic)]
#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]
use hive::node::Target;
use tokio::process::Command;

use crate::errors::HiveLibError;

pub mod hive;
mod nix;
mod nix_log;

#[cfg(test)]
mod test_macros;

#[cfg(test)]
mod test_support;

pub mod errors;

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

#[derive(Debug, Default, Clone, Copy)]
pub struct SubCommandModifiers {
    pub show_trace: bool,
}
