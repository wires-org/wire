// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright 2024-2025 wire Contributors

use std::collections::HashMap;

use tracing::instrument;

use crate::{
    EvalGoal, SubCommandModifiers,
    commands::{CommandArguments, Either, WireCommandChip, run_command, run_command_with_env},
    errors::HiveLibError,
    hive::{
        HiveLocation,
        node::{Context, Push},
    },
};

pub async fn push(context: &Context<'_>, push: Push<'_>) -> Result<(), HiveLibError> {
    let command_string = format!(
        "nix --extra-experimental-features nix-command \
        copy --substitute-on-destination --to ssh://{user}@{host} {path}",
        user = context.node.target.user,
        host = context.node.target.get_preferred_host()?,
        path = match push {
            Push::Derivation(drv) => format!("{drv} --derivation"),
            Push::Path(path) => path.clone(),
        }
    );

    let child = run_command_with_env(
        &CommandArguments::new(command_string, context.modifiers).nix(),
        HashMap::from([(
            "NIX_SSHOPTS".into(),
            context
                .node
                .target
                .create_ssh_opts(context.modifiers, false),
        )]),
    )?;

    child
        .wait_till_success()
        .await
        .map_err(|error| HiveLibError::NixCopyError {
            name: context.name.clone(),
            path: push.to_string(),
            error: Box::new(error),
        })?;

    Ok(())
}

/// Evaluates the hive in flakeref with regards to the given goal,
/// and returns stdout.
#[instrument(ret(level = tracing::Level::TRACE), skip_all)]
pub async fn evaluate_hive_attribute(
    location: &HiveLocation,
    goal: &EvalGoal<'_>,
    modifiers: SubCommandModifiers,
) -> Result<String, HiveLibError> {
    let attribute = match location {
        HiveLocation::Flake(uri) => {
            format!(
                "{uri}#wire --apply \"hive: {}\"",
                match goal {
                    EvalGoal::Inspect => "hive.inspect".to_string(),
                    EvalGoal::GetTopLevel(node) => format!("hive.topLevels.{node}"),
                }
            )
        }
        HiveLocation::HiveNix(path) => {
            format!(
                "--file {} {}",
                &path.to_string_lossy(),
                match goal {
                    EvalGoal::Inspect => "inspect".to_string(),
                    EvalGoal::GetTopLevel(node) => format!("topLevels.{node}"),
                }
            )
        }
    };

    let command_string = format!(
        "nix --extra-experimental-features nix-command \
        --extra-experimental-features flakes \
        eval --json {mods} {attribute}",
        mods = if modifiers.show_trace {
            "--show-trace"
        } else {
            ""
        },
    );

    let child = run_command(&CommandArguments::new(command_string, modifiers).nix())?;

    child
        .wait_till_success()
        .await
        .map_err(|source| HiveLibError::NixEvalError { attribute, source })
        .map(|x| match x {
            Either::Left((_, stdout)) | Either::Right((_, stdout)) => stdout,
        })
}

#[cfg(test)]
mod tests {
    use std::{assert_matches::assert_matches, collections::HashMap, env};

    use crate::{
        SubCommandModifiers,
        commands::{
            CommandArguments, WireCommandChip, common::push,
            noninteractive::non_interactive_command_with_env,
        },
        errors::CommandError,
        hive::node::{Context, Name, Node, Push},
        test_support::test_with_vm,
    };

    #[tokio::test]
    async fn push_to_vm() {
        let vm = test_with_vm();
        let mut node = Node::from_target(vm.target.clone());
        let name = Name("test".into());
        let mut context = Context::create_test_context(
            crate::hive::HiveLocation::Flake("in-test".to_string()),
            &name,
            &mut node,
        );
        context.modifiers = SubCommandModifiers {
            ssh_accept_host: true,
            ..Default::default()
        };

        let push_path = env::var("WIRE_PUSHABLE_PATH").unwrap();
        let to_push = Push::Path(&push_path);

        let child = non_interactive_command_with_env(
            &CommandArguments::new(format!("stat {push_path}"), context.modifiers)
                .on_target(Some(&context.node.target)),
            HashMap::new(),
        )
        .unwrap();

        assert_matches!(
            child.wait_till_success().await,
            Err(CommandError::CommandFailed { command_ran, logs, code, reason }) if logs.contains("No such file or directory")
        );

        push(&context, to_push).await.unwrap();

        let child = non_interactive_command_with_env(
            &CommandArguments::new(format!("stat {push_path}"), context.modifiers)
                .on_target(Some(&node.target)),
            HashMap::new(),
        )
        .unwrap();

        child.wait_till_success().await.unwrap();
    }
}
