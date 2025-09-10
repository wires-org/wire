use std::fmt::Display;

use async_trait::async_trait;
use tracing::instrument;

use crate::{
    EvalGoal, HiveLibError,
    commands::common::evaluate_hive_attribute,
    hive::node::{Context, ExecuteStep, Goal},
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
        let output = evaluate_hive_attribute(
            &ctx.hivepath,
            &EvalGoal::GetTopLevel(ctx.name),
            ctx.modifiers,
            ctx.clobber_lock.clone(),
        )
        .await?;

        ctx.state.evaluation = serde_json::from_str(&output).expect("failed to parse derivation");

        Ok(())
    }
}
