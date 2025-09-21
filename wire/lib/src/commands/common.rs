use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, Mutex},
};

use crate::{
    EvalGoal, SubCommandModifiers,
    commands::{
        ChildOutputMode, WireCommand, WireCommandChip, noninteractive::NonInteractiveCommand,
    },
    errors::{HiveInitializationError, HiveLibError},
    hive::{
        find_hive,
        node::{Name, Node, Push},
    },
};

pub async fn push(
    node: &Node,
    name: &Name,
    push: Push<'_>,
    modifiers: SubCommandModifiers,
    clobber_lock: Arc<Mutex<()>>,
) -> Result<(), HiveLibError> {
    let mut command =
        NonInteractiveCommand::spawn_new(None, ChildOutputMode::Nix, modifiers).await?;

    let command_string = format!(
        "nix --extra-experimental-features nix-command \
        copy --substitute-on-destination --to ssh://{user}@{host} {path}",
        user = node.target.user,
        host = node.target.get_preferred_host()?,
        path = match push {
            Push::Derivation(drv) => format!("{drv} --derivation"),
            Push::Path(path) => path.clone(),
        }
    );

    let child = command.run_command_with_env(
        command_string,
        false,
        HashMap::from([("NIX_SSHOPTS".into(), node.target.create_ssh_opts(modifiers))]),
        clobber_lock,
    )?;

    child
        .wait_till_success()
        .await
        .map_err(|error| HiveLibError::NixCopyError {
            name: name.clone(),
            path: push.to_string(),
            error: Box::new(error),
        })?;

    Ok(())
}

/// Evaluates the hive in path with regards to the given goal,
/// and returns stdout.
pub async fn evaluate_hive_attribute(
    path: &Path,
    goal: &EvalGoal<'_>,
    modifiers: SubCommandModifiers,
    clobber_lock: Arc<Mutex<()>>,
) -> Result<String, HiveLibError> {
    let canon_path =
        find_hive(&path.canonicalize().unwrap()).ok_or(HiveLibError::HiveInitializationError(
            HiveInitializationError::NoHiveFound(path.to_path_buf()),
        ))?;

    let mut command =
        NonInteractiveCommand::spawn_new(None, ChildOutputMode::Nix, modifiers).await?;
    let attribute = if canon_path.ends_with("flake.nix") {
        format!(
            "{}#wire --apply \"hive: {}\"",
            canon_path.to_str().unwrap(),
            match goal {
                EvalGoal::Inspect => "hive.inspect".to_string(),
                EvalGoal::GetTopLevel(node) => format!("hive.topLevels.{node}"),
            }
        )
    } else {
        format!(
            "--file {} {}",
            &canon_path.to_string_lossy(),
            match goal {
                EvalGoal::Inspect => "inspect".to_string(),
                EvalGoal::GetTopLevel(node) => format!("topLevels.{node}"),
            }
        )
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

    let child = command.run_command(command_string, false, clobber_lock)?;

    child
        .wait_till_success()
        .await
        .map_err(|source| HiveLibError::NixEvalError { attribute, source })
        .map(|(_, stdout)| stdout)
}
