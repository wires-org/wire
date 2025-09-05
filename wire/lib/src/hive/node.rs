#![allow(clippy::missing_errors_doc)]
use async_trait::async_trait;
use gethostname::gethostname;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{error, info, instrument, trace};

use crate::SubCommandModifiers;
use crate::commands::nonelevated::NonElevatedCommand;
use crate::commands::{ChildOutputMode, WireCommand, WireCommandChip};
use crate::errors::NetworkError;
use crate::hive::steps::keys::{Key, KeysStep, PushKeyAgentStep, UploadKeyAt};
use crate::hive::steps::ping::PingStep;

use super::HiveLibError;
use super::steps::activate::SwitchToConfigurationStep;

#[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq, derive_more::Display)]
pub struct Name(pub Arc<str>);

#[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Target {
    pub hosts: Vec<Arc<str>>,
    pub user: Arc<str>,
    pub port: u32,

    #[serde(skip)]
    current_host: usize,
}

#[cfg(test)]
impl Default for Target {
    fn default() -> Self {
        Target {
            hosts: vec!["NAME".into()],
            user: "root".into(),
            port: 22,
            current_host: 0,
        }
    }
}

impl Target {
    pub fn get_preffered_host(&self) -> Result<&Arc<str>, HiveLibError> {
        self.hosts
            .get(self.current_host)
            .ok_or(HiveLibError::NetworkError(NetworkError::HostsExhausted))
    }

    pub fn host_failed(&mut self) {
        self.current_host += 1;
    }

    #[cfg(test)]
    pub fn from_host(host: &str) -> Self {
        Target {
            hosts: vec![host.into()],
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Node {
    #[serde(rename = "target")]
    pub target: Target,

    #[serde(rename = "buildOnTarget")]
    pub build_remotely: bool,

    #[serde(rename = "allowLocalDeployment")]
    pub allow_local_deployment: bool,

    #[serde(default)]
    pub tags: im::HashSet<String>,

    #[serde(rename(deserialize = "_keys", serialize = "keys"))]
    pub keys: im::Vector<Key>,

    #[serde(rename(deserialize = "_hostPlatform", serialize = "host_platform"))]
    pub host_platform: Arc<str>,
}

#[cfg(test)]
impl Default for Node {
    fn default() -> Self {
        Node {
            target: Target::default(),
            keys: im::Vector::new(),
            tags: im::HashSet::new(),
            allow_local_deployment: true,
            build_remotely: false,
            host_platform: "x86_64-linux".into(),
        }
    }
}

impl Node {
    #[cfg(test)]
    pub fn from_host(host: &str) -> Self {
        Node {
            target: Target::from_host(host),
            ..Default::default()
        }
    }

    pub async fn ping(&self, clobber_lock: Arc<Mutex<()>>) -> Result<(), HiveLibError> {
        let host = self.target.get_preffered_host()?;

        let command_string = format!(
            "nix --extra-experimental-features nix-command \
            store ping --store ssh://{}@{}",
            self.target.user, host
        );

        let mut command = NonElevatedCommand::spawn_new(None, ChildOutputMode::Nix).await?;
        let output = command.run_command_with_env(
            command_string,
            false,
            HashMap::from([("NIX_SSHOPTS".into(), format!("-p {}", self.target.port))]),
            clobber_lock,
        )?;

        output.wait_till_success().await.map_err(|source| {
            HiveLibError::NetworkError(NetworkError::HostUnreachable {
                host: host.to_string(),
                source,
            })
        })?;

        Ok(())
    }
}

pub fn should_apply_locally(allow_local_deployment: bool, name: &str) -> bool {
    *name == *gethostname() && allow_local_deployment
}

#[derive(derive_more::Display)]
pub enum Push<'a> {
    Derivation(&'a Derivation),
    Path(&'a String),
}

#[derive(Deserialize, Debug)]
pub struct Derivation(String);

impl Display for Derivation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f).and_then(|()| write!(f, "^*"))
    }
}

#[derive(derive_more::Display, Debug, Clone, Copy)]
pub enum SwitchToConfigurationGoal {
    Switch,
    Boot,
    Test,
    DryActivate,
}

#[derive(derive_more::Display, Clone, Copy)]
pub enum Goal {
    SwitchToConfiguration(SwitchToConfigurationGoal),
    Build,
    Push,
    Keys,
}

#[async_trait]
pub trait ExecuteStep: Send + Sync + Display {
    async fn execute(&self, ctx: &mut Context<'_>) -> Result<(), HiveLibError>;

    fn should_execute(&self, context: &Context) -> bool;
}

#[derive(Default)]
pub struct StepState {
    pub evaluation: Option<Derivation>,
    pub build: Option<String>,
    pub key_agent_directory: Option<String>,
}

pub struct Context<'a> {
    pub name: &'a Name,
    pub node: &'a mut Node,
    pub hivepath: PathBuf,
    pub modifiers: SubCommandModifiers,
    pub no_keys: bool,
    pub state: StepState,
    pub goal: Goal,
    pub reboot: bool,
    pub clobber_lock: Arc<Mutex<()>>,
}

pub struct GoalExecutor<'a> {
    steps: Vec<Box<dyn ExecuteStep>>,
    context: Context<'a>,
}

impl<'a> GoalExecutor<'a> {
    pub fn new(context: Context<'a>) -> Self {
        Self {
            steps: vec![
                Box::new(PingStep),
                Box::new(PushKeyAgentStep),
                Box::new(KeysStep {
                    filter: UploadKeyAt::NoFilter,
                }),
                Box::new(KeysStep {
                    filter: UploadKeyAt::PreActivation,
                }),
                Box::new(super::steps::evaluate::Step),
                Box::new(super::steps::push::EvaluatedOutputStep),
                Box::new(super::steps::build::Step),
                Box::new(super::steps::push::BuildOutputStep),
                Box::new(SwitchToConfigurationStep),
                Box::new(KeysStep {
                    filter: UploadKeyAt::PostActivation,
                }),
            ],
            context,
        }
    }

    #[instrument(skip_all, name = "goal", fields(node = %self.context.name))]
    pub async fn execute(mut self) -> Result<(), HiveLibError> {
        let steps = self
            .steps
            .iter()
            .filter(|step| step.should_execute(&self.context))
            .inspect(|step| trace!("Will execute step `{step}` for {}", self.context.name))
            .collect::<Vec<_>>();

        for step in steps {
            info!("Executing step `{step}`");

            step.execute(&mut self.context).await.inspect_err(|_| {
                error!("Failed to execute `{step}`");
            })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_test_path, hive::Hive, test_support::get_clobber_lock};
    use std::{collections::HashMap, env};

    #[tokio::test]
    #[cfg_attr(feature = "no_web_tests", ignore)]
    async fn default_values_match() {
        let mut path = get_test_path!();

        let hive = Hive::new_from_path(&path, SubCommandModifiers::default(), get_clobber_lock())
            .await
            .unwrap();

        let node = Node::default();

        let mut nodes = HashMap::new();
        nodes.insert(Name("NAME".into()), node);

        path.push("hive.nix");

        assert_eq!(
            hive,
            Hive {
                nodes,
                schema: Hive::SCHEMA_VERSION
            }
        );
    }
}
