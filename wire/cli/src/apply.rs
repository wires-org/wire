use futures::{FutureExt, StreamExt};
use itertools::{Either, Itertools};
use lib::hive::Hive;
use lib::hive::node::{Context, GoalExecutor, Name, StepState};
use lib::{SubCommandModifiers, errors::HiveLibError};
use miette::{Diagnostic, Result};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{Span, error, info, instrument};

use crate::cli::{ApplyArgs, ApplyTarget};

#[derive(Debug, Error, Diagnostic)]
#[error("node {} failed to apply", .0)]
struct NodeError(
    Name,
    #[source]
    #[diagnostic_source]
    HiveLibError,
);

#[derive(Debug, Error, Diagnostic)]
#[error("{} node(s) failed to apply.", .0.len())]
struct NodeErrors(#[related] Vec<NodeError>);

#[instrument(skip_all, fields(goal = %args.goal, on = %args.on.iter().join(", ")))]
pub async fn apply(
    hive: &mut Hive,
    args: ApplyArgs,
    path: PathBuf,
    modifiers: SubCommandModifiers,
    clobber_lock: Arc<Mutex<()>>,
) -> Result<()> {
    let header_span = Span::current();

    // Respect user's --always-build-local arg
    hive.force_always_local(args.always_build_local)?;

    let header_span_enter = header_span.enter();

    let (tags, names) = args.on.iter().fold(
        (HashSet::new(), HashSet::new()),
        |(mut tags, mut names), target| {
            match target {
                ApplyTarget::Tag(tag) => tags.insert(tag.clone()),
                ApplyTarget::Node(name) => names.insert(name.clone()),
            };
            (tags, names)
        },
    );

    let mut set = hive
        .nodes
        .iter_mut()
        .filter(|(name, node)| {
            args.on.is_empty()
                || names.contains(name)
                || node.tags.iter().any(|tag| tags.contains(tag))
        })
        .map(|node| {
            let path = path.clone();

            info!("Resolved {:?} to include {}", args.on, node.0);

            let context = Context {
                node: node.1,
                name: node.0,
                goal: args.goal.clone().try_into().unwrap(),
                state: StepState::default(),
                no_keys: args.no_keys,
                hivepath: path,
                modifiers,
                reboot: args.reboot,
                clobber_lock: clobber_lock.clone(),
            };

            GoalExecutor::new(context)
                .execute()
                .map(move |result| (node.0, result))
        })
        .peekable();

    if set.peek().is_none() {
        error!("There are no nodes selected for deployment");
    }

    let futures = futures::stream::iter(set).buffer_unordered(args.parallel);
    let result = futures.collect::<Vec<_>>().await;
    let (successful, errors): (Vec<_>, Vec<_>) =
        result
            .into_iter()
            .partition_map(|(name, result)| match result {
                Ok(..) => Either::Left(name),
                Err(err) => Either::Right((name, err)),
            });

    if !successful.is_empty() {
        info!(
            "Successfully applied goal to {} node(s): {:?}",
            successful.len(),
            successful
        );
    }

    std::mem::drop(header_span_enter);
    std::mem::drop(header_span);

    if !errors.is_empty() {
        return Err(NodeErrors(
            errors
                .into_iter()
                .map(|(name, error)| NodeError(name.clone(), error))
                .collect(),
        )
        .into());
    }

    Ok(())
}
