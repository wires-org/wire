#![allow(clippy::missing_errors_doc)]
use async_trait::async_trait;
use gethostname::gethostname;
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::process::Command;
use tracing::{error, info, instrument, trace, Instrument, Span};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use crate::nix::StreamTracing;
use crate::SubCommandModifiers;

use super::key::{Key, PushAgentStep, UploadKeyAt, UploadKeyStep};
use super::steps::activate::SwitchToConfigurationStep;
use super::HiveLibError;

#[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq, derive_more::Display)]
pub struct Name(pub Arc<str>);

#[derive(Serialize, Deserialize, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Target {
    #[serde(rename = "host")]
    pub host: Arc<str>,

    #[serde(rename = "user")]
    pub user: Arc<str>,

    #[serde(rename = "port")]
    pub port: u32,
}

impl Default for Target {
    fn default() -> Self {
        Target {
            host: "NAME".into(),
            user: "root".into(),
            port: 22,
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
}

impl Default for Node {
    fn default() -> Self {
        Node {
            target: Target::default(),
            keys: im::Vector::new(),
            tags: im::HashSet::new(),
            allow_local_deployment: true,
            build_remotely: false,
        }
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
    pub agent_directory: Option<String>,
}

pub struct Context<'a> {
    pub name: &'a Name,
    pub node: &'a Node,
    pub hivepath: PathBuf,
    pub modifiers: SubCommandModifiers,
    pub no_keys: bool,
    pub state: StepState,
    pub goal: Goal,
}

pub struct GoalExecutor<'a> {
    steps: Vec<Box<dyn ExecuteStep>>,
    context: Context<'a>,
}

impl<'a> GoalExecutor<'a> {
    pub fn new(context: Context<'a>) -> Self {
        Self {
            steps: vec![
                Box::new(PushAgentStep),
                Box::new(UploadKeyStep {
                    moment: UploadKeyAt::AnyOpportunity,
                }),
                Box::new(UploadKeyStep {
                    moment: UploadKeyAt::PreActivation,
                }),
                Box::new(super::steps::evaluate::Step),
                Box::new(super::steps::push::EvaluatedOutputStep),
                Box::new(super::steps::build::Step),
                Box::new(super::steps::push::BuildOutputStep),
                Box::new(SwitchToConfigurationStep),
                Box::new(UploadKeyStep {
                    moment: UploadKeyAt::PostActivation,
                }),
            ],
            context,
        }
    }

    #[instrument(skip_all, name = "goal", fields(node = %self.context.name))]
    pub async fn execute(mut self, span: Span) -> Result<(), HiveLibError> {
        let steps = self
            .steps
            .iter()
            .filter(|step| step.should_execute(&self.context))
            .inspect(|step| trace!("Will execute step `{step}` for {}", self.context.name))
            .collect::<Vec<_>>();

        span.pb_inc_length(steps.len().try_into().unwrap());

        for step in steps {
            info!("Executing step `{step}`");

            step.execute(&mut self.context).await.inspect_err(|_| {
                error!("Failed to execute `{step}`");
            })?;

            span.pb_inc(1);
        }

        Ok(())
    }
}

pub async fn push(node: &Node, name: &Name, push: Push<'_>) -> Result<(), HiveLibError> {
    let mut command = Command::new("nix");

    command
        .args(["--extra-experimental-features", "nix-command"])
        .arg("copy")
        .arg("--substitute-on-destination")
        .arg("--to")
        .arg(format!("ssh://{}@{}", node.target.user, node.target.host))
        .env("NIX_SSHOPTS", format!("-p {}", node.target.port));

    match push {
        Push::Derivation(drv) => command.args([drv.to_string(), "--derivation".to_string()]),
        Push::Path(path) => command.arg(path),
    };

    let (status, _stdout, stderr_vec) = command.execute(true).in_current_span().await?;

    if !status.success() {
        return Err(HiveLibError::NixCopyError(name.clone(), stderr_vec));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{get_test_path, hive::Hive};
    use std::{collections::HashMap, env};

    #[tokio::test]
    #[cfg_attr(feature = "no_web_tests", ignore)]
    async fn default_values_match() {
        let mut path = get_test_path!();

        let hive = Hive::new_from_path(&path, SubCommandModifiers::default())
            .await
            .unwrap();

        let node = Node::default();

        let mut nodes = HashMap::new();
        nodes.insert(Name("NAME".into()), node);

        path.push("hive.nix");

        assert_eq!(hive, Hive { nodes, path });
    }
}
