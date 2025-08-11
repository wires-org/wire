use std::fmt::Display;

use async_trait::async_trait;
use tracing::{Instrument, instrument};

use crate::{
    HiveLibError,
    hive::node::{Context, Derivation, ExecuteStep, Goal},
    nix::{EvalGoal, StreamTracing, get_eval_command},
};

pub struct Step;

impl Display for Step {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Evaluate the node")
    }
}

#[async_trait]
impl ExecuteStep for Step {
    fn should_execute(&self, ctx: &Context) -> bool {
        !matches!(ctx.goal, Goal::Keys)
    }

    #[instrument(skip_all, name = "eval")]
    async fn execute(&self, ctx: &mut Context<'_>) -> Result<(), HiveLibError> {
        let command = get_eval_command(
            &ctx.hivepath,
            &EvalGoal::GetTopLevel(ctx.name),
            ctx.modifiers,
        );

        let (status, stdout_vec, stderr) = command?.execute(true).in_current_span().await?;

        if status.success() {
            let stdout: Vec<String> = stdout_vec
                .into_iter()
                .map(|l| l.to_string())
                .filter(|s| !s.is_empty())
                .collect();

            let derivation: Derivation =
                serde_json::from_str(&stdout.join("\n")).expect("failed to parse derivation");

            ctx.state.evaluation = Some(derivation);

            return Ok(());
        }

        Err(HiveLibError::NixEvalInternalError(ctx.name.clone(), stderr))
    }
}
