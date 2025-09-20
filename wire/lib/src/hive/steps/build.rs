use std::fmt::Display;

use tracing::{info, instrument};

use crate::{
    HiveLibError,
    commands::{
        ChildOutputMode, WireCommand, WireCommandChip, noninteractive::NonInteractiveCommand,
    },
    hive::node::{Context, ExecuteStep, Goal},
};

#[derive(Debug, PartialEq)]
pub struct Build;

impl Display for Build {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Build the node")
    }
}

impl ExecuteStep for Build {
    fn should_execute(&self, ctx: &Context) -> bool {
        !matches!(ctx.goal, Goal::Keys | Goal::Push)
    }

    #[instrument(skip_all, name = "build")]
    async fn execute(&self, ctx: &mut Context<'_>) -> Result<(), HiveLibError> {
        let top_level = ctx.state.evaluation.as_ref().unwrap();

        let command_string = format!(
            "nix --extra-experimental-features nix-command \
            build --print-build-logs --print-out-paths {top_level}"
        );

        let mut command = NonInteractiveCommand::spawn_new(
            if ctx.node.build_remotely {
                Some(&ctx.node.target)
            } else {
                None
            },
            ChildOutputMode::Nix,
            ctx.modifiers,
        )
        .await?;

        let (_, stdout) = command
            .run_command(command_string, false, ctx.clobber_lock.clone())?
            .wait_till_success()
            .await
            .map_err(|source| HiveLibError::NixBuildError {
                name: ctx.name.clone(),
                source,
            })?;

        info!("Built output: {stdout:?}");
        ctx.state.build = Some(stdout);

        Ok(())
    }
}
