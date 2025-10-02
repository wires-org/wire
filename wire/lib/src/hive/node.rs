#![allow(clippy::missing_errors_doc)]
use enum_dispatch::enum_dispatch;
use gethostname::gethostname;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing::{error, info, instrument, trace};

use crate::SubCommandModifiers;
use crate::commands::noninteractive::NonInteractiveCommand;
use crate::commands::{ChildOutputMode, WireCommand, WireCommandChip};
use crate::errors::NetworkError;
use crate::hive::steps::build::Build;
use crate::hive::steps::evaluate::Evaluate;
use crate::hive::steps::keys::{Key, Keys, PushKeyAgent, UploadKeyAt};
use crate::hive::steps::ping::Ping;
use crate::hive::steps::push::{PushBuildOutput, PushEvaluatedOutput};

use super::HiveLibError;
use super::steps::activate::SwitchToConfiguration;

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

impl Target {
    pub fn create_ssh_opts(&self, modifiers: SubCommandModifiers) -> String {
        format!(
            "-p {} {}",
            self.port,
            if modifiers.ssh_accept_host {
                "-o StrictHostKeyChecking=no"
            } else {
                "-o StrictHostKeyChecking=accept-new"
            }
        )
    }

    pub fn create_ssh_args(
        &self,
        modifiers: SubCommandModifiers,
    ) -> Result<Vec<String>, HiveLibError> {
        let mut vector = vec![
            "-l".to_string(),
            self.user.to_string(),
            self.get_preferred_host()?.to_string(),
            "-p".to_string(),
            self.port.to_string(),
        ];

        vector.extend([
            "-o".to_string(),
            format!(
                "StrictHostKeyChecking {}",
                if modifiers.ssh_accept_host {
                    "no"
                } else {
                    "accept-new"
                }
            )
            .to_string(),
        ]);

        Ok(vector)
    }
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

#[cfg(test)]
impl<'a> Context<'a> {
    fn create_test_context(
        hivepath: std::path::PathBuf,
        name: &'a Name,
        node: &'a mut Node,
    ) -> Self {
        use crate::test_support::get_clobber_lock;

        Context {
            name,
            node,
            hivepath,
            modifiers: SubCommandModifiers::default(),
            no_keys: false,
            state: StepState::default(),
            goal: Goal::SwitchToConfiguration(SwitchToConfigurationGoal::Switch),
            reboot: false,
            clobber_lock: get_clobber_lock(),
        }
    }
}

impl Target {
    pub fn get_preferred_host(&self) -> Result<&Arc<str>, HiveLibError> {
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

    pub async fn ping(
        &self,
        modifiers: SubCommandModifiers,
        clobber_lock: Arc<Mutex<()>>,
    ) -> Result<(), HiveLibError> {
        let host = self.target.get_preferred_host()?;

        let command_string = format!(
            "nix --extra-experimental-features nix-command \
            store ping --store ssh://{}@{}",
            self.target.user, host
        );

        let mut command =
            NonInteractiveCommand::spawn_new(None, ChildOutputMode::Nix, modifiers).await?;
        let output = command.run_command_with_env(
            command_string,
            false,
            HashMap::from([("NIX_SSHOPTS".into(), self.target.create_ssh_opts(modifiers))]),
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

#[enum_dispatch]
pub(crate) trait ExecuteStep: Send + Sync + Display + std::fmt::Debug {
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

#[enum_dispatch(ExecuteStep)]
#[derive(Debug, PartialEq)]
enum Step {
    Ping,
    PushKeyAgent,
    Keys,
    Evaluate,
    PushEvaluatedOutput,
    Build,
    PushBuildOutput,
    SwitchToConfiguration,
}

impl Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ping(step) => step.fmt(f),
            Self::PushKeyAgent(step) => step.fmt(f),
            Self::Keys(step) => step.fmt(f),
            Self::Evaluate(step) => step.fmt(f),
            Self::PushEvaluatedOutput(step) => step.fmt(f),
            Self::Build(step) => step.fmt(f),
            Self::PushBuildOutput(step) => step.fmt(f),
            Self::SwitchToConfiguration(step) => step.fmt(f),
        }
    }
}

pub struct GoalExecutor<'a> {
    steps: Vec<Step>,
    context: Context<'a>,
}

impl<'a> GoalExecutor<'a> {
    pub fn new(context: Context<'a>) -> Self {
        Self {
            steps: vec![
                Step::Ping(Ping),
                Step::PushKeyAgent(PushKeyAgent),
                Step::Keys(Keys {
                    filter: UploadKeyAt::NoFilter,
                }),
                Step::Keys(Keys {
                    filter: UploadKeyAt::PreActivation,
                }),
                Step::Evaluate(super::steps::evaluate::Evaluate),
                Step::PushEvaluatedOutput(super::steps::push::PushEvaluatedOutput),
                Step::Build(super::steps::build::Build),
                Step::PushBuildOutput(super::steps::push::PushBuildOutput),
                Step::SwitchToConfiguration(SwitchToConfiguration),
                Step::Keys(Keys {
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
            .inspect(|step| {
                trace!("Will execute step `{step}` for {}", self.context.name);
            })
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
    use crate::{function_name, get_test_path, hive::Hive, test_support::get_clobber_lock};
    use std::{collections::HashMap, env};

    fn get_steps(goal_executor: GoalExecutor) -> std::vec::Vec<Step> {
        goal_executor
            .steps
            .into_iter()
            .filter(|step| step.should_execute(&goal_executor.context))
            .collect::<Vec<_>>()
    }

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

    #[tokio::test]
    async fn order_build_locally() {
        let path = get_test_path!();
        let mut node = Node {
            build_remotely: false,
            ..Default::default()
        };
        let name = &Name(function_name!().into());
        let executor = GoalExecutor::new(Context::create_test_context(path, name, &mut node));
        let steps = get_steps(executor);

        assert_eq!(
            steps,
            vec![
                Ping.into(),
                PushKeyAgent.into(),
                Keys {
                    filter: UploadKeyAt::PreActivation
                }
                .into(),
                crate::hive::steps::evaluate::Evaluate.into(),
                crate::hive::steps::build::Build.into(),
                crate::hive::steps::push::PushBuildOutput.into(),
                SwitchToConfiguration.into(),
                Keys {
                    filter: UploadKeyAt::PostActivation
                }
                .into()
            ]
        );
    }

    #[tokio::test]
    async fn order_keys_only() {
        let path = get_test_path!();
        let mut node = Node::default();
        let name = &Name(function_name!().into());
        let mut context = Context::create_test_context(path, name, &mut node);

        context.goal = Goal::Keys;

        let executor = GoalExecutor::new(context);
        let steps = get_steps(executor);

        assert_eq!(
            steps,
            vec![
                Ping.into(),
                PushKeyAgent.into(),
                Keys {
                    filter: UploadKeyAt::NoFilter
                }
                .into(),
            ]
        );
    }

    #[tokio::test]
    async fn order_build_only() {
        let path = get_test_path!();
        let mut node = Node::default();
        let name = &Name(function_name!().into());
        let mut context = Context::create_test_context(path, name, &mut node);

        context.goal = Goal::Build;

        let executor = GoalExecutor::new(context);
        let steps = get_steps(executor);

        assert_eq!(
            steps,
            vec![
                Ping.into(),
                crate::hive::steps::evaluate::Evaluate.into(),
                crate::hive::steps::build::Build.into(),
                crate::hive::steps::push::PushBuildOutput.into(),
            ]
        );
    }

    #[tokio::test]
    async fn order_push_only() {
        let path = get_test_path!();
        let mut node = Node::default();
        let name = &Name(function_name!().into());
        let mut context = Context::create_test_context(path, name, &mut node);

        context.goal = Goal::Push;

        let executor = GoalExecutor::new(context);
        let steps = get_steps(executor);

        assert_eq!(
            steps,
            vec![
                Ping.into(),
                crate::hive::steps::evaluate::Evaluate.into(),
                crate::hive::steps::push::PushEvaluatedOutput.into(),
            ]
        );
    }

    #[tokio::test]
    async fn order_remote_build() {
        let path = get_test_path!();
        let mut node = Node {
            build_remotely: true,
            ..Default::default()
        };

        let name = &Name(function_name!().into());
        let executor = GoalExecutor::new(Context::create_test_context(path, name, &mut node));
        let steps = get_steps(executor);

        assert_eq!(
            steps,
            vec![
                Ping.into(),
                PushKeyAgent.into(),
                Keys {
                    filter: UploadKeyAt::PreActivation
                }
                .into(),
                crate::hive::steps::evaluate::Evaluate.into(),
                crate::hive::steps::push::PushEvaluatedOutput.into(),
                crate::hive::steps::build::Build.into(),
                SwitchToConfiguration.into(),
                Keys {
                    filter: UploadKeyAt::PostActivation
                }
                .into()
            ]
        );
    }
}
