use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    errors::{DetachedError, HiveLibError},
    hive::node::Target,
    nix_log::{Action, Internal, NixLog, Trace},
};

pub(crate) mod common;
pub(crate) mod elevated;
pub(crate) mod nonelevated;

#[derive(Copy, Clone)]
pub(crate) enum ChildOutputMode {
    Raw,
    Nix,
}

pub(crate) trait WireCommand<'target>: Sized {
    type ChildChip;

    async fn spawn_new(
        target: Option<&'target Target>,
        output_mode: ChildOutputMode,
    ) -> Result<Self, HiveLibError>;

    fn run_command<S: AsRef<str>>(
        &mut self,
        command_string: S,
        keep_stdin_open: bool,
        clobber_lock: Arc<Mutex<()>>,
    ) -> Result<Self::ChildChip, HiveLibError> {
        self.run_command_with_env(
            command_string,
            keep_stdin_open,
            std::collections::HashMap::new(),
            clobber_lock,
        )
    }

    fn run_command_with_env<S: AsRef<str>>(
        &mut self,
        command_string: S,
        keep_stdin_open: bool,
        args: HashMap<String, String>,
        clobber_lock: Arc<Mutex<()>>,
    ) -> Result<Self::ChildChip, HiveLibError>;
}

pub(crate) trait WireCommandChip {
    type ExitStatus;

    async fn wait_till_success(self) -> Result<Self::ExitStatus, DetachedError>;
    async fn write_stdin(&self, data: Vec<u8>) -> Result<(), HiveLibError>;
}

impl ChildOutputMode {
    fn trace(self, line: String) -> Option<NixLog> {
        let log = match self {
            ChildOutputMode::Nix => {
                let log =
                    serde_json::from_str::<Internal>(line.strip_prefix("@nix ").unwrap_or(&line))
                        .map(NixLog::Internal)
                        .unwrap_or(NixLog::Raw(line));

                // Throw out stop logs
                if let NixLog::Internal(Internal {
                    action: Action::Stop,
                }) = log
                {
                    return None;
                }

                log
            }
            Self::Raw => NixLog::Raw(line),
        };

        log.trace();

        Some(log)
    }
}
