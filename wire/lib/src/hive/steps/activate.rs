use std::{fmt::Display, process::Output};

use async_trait::async_trait;
use tokio::process::Command;
use tracing::{Instrument, error, info, instrument, warn};
use tracing_indicatif::suspend_tracing_indicatif;

use crate::{
    HiveLibError, NetworkError, create_ssh_command,
    hive::node::{Context, ExecuteStep, Goal, SwitchToConfigurationGoal, should_apply_locally},
    nix::StreamTracing,
};

pub struct SwitchToConfigurationStep;

impl Display for SwitchToConfigurationStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Switch to configuration")
    }
}

pub(crate) fn get_elevation(reason: &str) -> Result<Output, HiveLibError> {
    info!("Attempting to elevate for {reason}.");
    suspend_tracing_indicatif(|| {
        let mut command = std::process::Command::new("sudo");
        command.arg("-v").output()
    })
    .map_err(HiveLibError::FailedToElevate)
}

pub async fn wait_for_ping(ctx: &Context<'_>) -> Result<(), HiveLibError> {
    for num in 0..3 {
        warn!(
            "Trying to ping {host} (attempt {num}/3)",
            host = ctx.node.target.get_preffered_host()?
        );

        if ctx.node.ping().await.is_ok() {
            info!(
                "Regained connection to {name} via {host}",
                name = ctx.name,
                host = ctx.node.target.get_preffered_host()?
            );

            return Ok(());
        }
    }

    Err(HiveLibError::NetworkError(NetworkError::HostUnreachable(
        ctx.node.target.get_preffered_host()?.to_string(),
    )))
}

#[async_trait]
impl ExecuteStep for SwitchToConfigurationStep {
    fn should_execute(&self, ctx: &Context) -> bool {
        matches!(ctx.goal, Goal::SwitchToConfiguration(..))
    }

    #[instrument(skip_all, name = "switch")]
    async fn execute(&self, ctx: &mut Context<'_>) -> Result<(), HiveLibError> {
        let built_path = ctx.state.build.as_ref().unwrap();

        let Goal::SwitchToConfiguration(goal) = &ctx.goal else {
            unreachable!("Cannot reach as guarded by should_execute")
        };

        if !matches!(
            goal,
            SwitchToConfigurationGoal::DryActivate | SwitchToConfigurationGoal::Boot
        ) {
            info!("Setting profiles in anticipation for switch-to-configuration {goal}");

            let mut env_command =
                if should_apply_locally(ctx.node.allow_local_deployment, &ctx.name.to_string()) {
                    // Refresh sudo timeout
                    warn!("Running nix-env ON THIS MACHINE for node {0}", ctx.name);
                    get_elevation("nix-env")?;
                    let mut command = Command::new("sudo");
                    command.arg("nix-env");
                    command
                } else {
                    let mut command = create_ssh_command(&ctx.node.target, true)?;
                    command.arg("nix-env");
                    command
                };

            env_command.args(["-p", "/nix/var/nix/profiles/system/", "--set", built_path]);

            let (status, _, stderr_vec) = env_command.execute(true).in_current_span().await?;

            if !status.success() {
                let stderr: Vec<String> = stderr_vec
                    .into_iter()
                    .map(|l| l.to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                return Err(HiveLibError::NixEnvError(ctx.name.clone(), stderr));
            }

            info!("Set system profile");
        }

        info!("Running switch-to-configuration {goal}");

        let cmd = format!("{built_path}/bin/switch-to-configuration");

        let mut command =
            if should_apply_locally(ctx.node.allow_local_deployment, &ctx.name.to_string()) {
                // Refresh sudo timeout
                warn!(
                    "Running switch-to-configuration {goal:?} ON THIS MACHINE for node {0}",
                    ctx.name
                );
                get_elevation("switch-to-configuration")?;
                let mut command = Command::new("sudo");
                command.arg(cmd);
                command
            } else {
                let mut command = create_ssh_command(&ctx.node.target, true)?;
                command.arg(cmd);
                command
            };

        command.arg(match goal {
            SwitchToConfigurationGoal::Switch => "switch",
            SwitchToConfigurationGoal::Boot => "boot",
            SwitchToConfigurationGoal::Test => "test",
            SwitchToConfigurationGoal::DryActivate => "dry-activate",
        });

        let (status, _, stderr_vec) = command.execute(true).in_current_span().await?;

        if status.success() {
            if !ctx.reboot {
                return Ok(());
            }

            if should_apply_locally(ctx.node.allow_local_deployment, &ctx.name.to_string()) {
                error!("Refusing to reboot local machine!");

                return Ok(());
            }

            warn!("Rebooting {name}!", name = ctx.name);

            let mut reboot = {
                let mut command = create_ssh_command(&ctx.node.target, true)?;
                command.args(["reboot", "now"]);
                command
            };

            // consume result, impossible to know if the machine failed to reboot or we
            // simply disconnected
            let _ = reboot.output().await;

            info!("Rebooted {name}, waiting to reconnect...", name = ctx.name);

            if wait_for_ping(ctx).await.is_ok() {
                return Ok(());
            }

            error!(
                "Failed to get regain connection to {name} via {host} after reboot.",
                name = ctx.name,
                host = ctx.node.target.get_preffered_host()?
            );

            return Err(HiveLibError::NetworkError(
                NetworkError::HostUnreachableAfterReboot(
                    ctx.node.target.get_preffered_host()?.to_string(),
                ),
            ));
        }

        let stderr: Vec<String> = stderr_vec
            .into_iter()
            .map(|l| l.to_string())
            .filter(|s| !s.is_empty())
            .collect();

        warn!(
            "Activation command for {name} exited unsuccessfully.",
            name = ctx.name
        );

        // Bail if the command couldn't of broken the system
        // and don't try to regain connection to localhost
        if matches!(goal, SwitchToConfigurationGoal::DryActivate)
            || should_apply_locally(ctx.node.allow_local_deployment, &ctx.name.to_string())
        {
            return Err(HiveLibError::SwitchToConfigurationError(
                *goal,
                ctx.name.clone(),
                stderr,
            ));
        }

        if wait_for_ping(ctx).await.is_ok() {
            return Ok(());
        }

        error!(
            "Failed to get regain connection to {name} via {host} after {goal} activation.",
            name = ctx.name,
            host = ctx.node.target.get_preffered_host()?
        );

        return Err(HiveLibError::SwitchToConfigurationError(
            *goal,
            ctx.name.clone(),
            stderr,
        ));
    }
}
