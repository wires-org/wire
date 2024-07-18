use clap::{Parser, Subcommand, ValueEnum};
use lib::hive::node::NodeName;

use std::{fmt::Display, sync::Arc};

#[derive(Parser)]
#[command(
    name = "wire",
    bin_name = "wire",
    about = "a tool to deploy nixos systems"
)]
pub struct WireCli {
    #[command(subcommand)]
    pub command: Commands,

    #[command(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,

    /// Path to directory containing hive
    #[arg(short, long, global = true, default_value = std::env::current_dir().unwrap().into_os_string())]
    pub path: std::path::PathBuf,
}

#[derive(Clone, Debug)]
pub enum ApplyTarget {
    Node(NodeName),
    Tag(String),
}

impl From<String> for ApplyTarget {
    fn from(value: String) -> Self {
        match value.starts_with("@") {
            true => ApplyTarget::Tag(value[1..].to_string()),
            false => ApplyTarget::Node(NodeName(Arc::from(value.as_str()))),
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    Apply {
        #[arg(value_enum, default_value_t)]
        goal: Goal,

        /// List of literal node names or `@` prefixed tags.
        #[arg(short, long)]
        on: Vec<ApplyTarget>,
    },
    /// Inspect hive
    Inspect {
        /// Include liveliness
        #[arg(short, long, default_value_t = false)]
        online: bool,

        /// Return in JSON format
        #[arg(short, long, default_value_t = false)]
        json: bool,
    },
    /// Inspect log of builds
    Log {
        /// Host identifier
        #[arg()]
        host: String,
        /// Reverse-index of log. 0 is the latest
        #[arg(default_value_t = 0)]
        index: i32,
    },
}

#[derive(Clone, Debug, Default, ValueEnum, Display)]
pub enum Goal {
    #[default]
    Switch,
    /// Build system profiles
    Build,
    /// Copy closures to remote hosts
    Push,
    /// Push deployment keys to remote hosts
    Keys,
    /// Activate system profile on next boot
    Boot,
    Test,
    DryActivate,
}
