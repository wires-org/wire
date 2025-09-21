#![deny(clippy::pedantic)]
#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]
#![feature(assert_matches)]

use crate::{errors::HiveLibError, hive::node::Name};

pub mod commands;
pub mod hive;
mod nix_log;

#[cfg(test)]
mod test_macros;

#[cfg(test)]
mod test_support;

pub mod errors;

#[derive(Debug, Default, Clone, Copy)]
pub struct SubCommandModifiers {
    pub show_trace: bool,
    pub non_interactive: bool,
    pub ssh_accept_host: bool,
}

pub enum EvalGoal<'a> {
    Inspect,
    GetTopLevel(&'a Name),
}
