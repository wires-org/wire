use futures::StreamExt;
use indicatif::ProgressStyle;
use itertools::Itertools;
use lib::hive::node::{Evaluatable, Goal};
use lib::hive::Hive;
use lib::{HiveLibError, SubCommandModifiers};
use std::collections::HashSet;
use tracing::{error, info, instrument, Span};
use tracing_indicatif::span_ext::IndicatifSpanExt;

use crate::cli::ApplyTarget;

#[instrument(skip_all, fields(goal = %goal, on = %on.iter().join(", ")))]
pub async fn apply(
    hive: &mut Hive,
    goal: Goal,
    on: Vec<ApplyTarget>,
    parallel: usize,
    no_keys: bool,
    always_local: Vec<String>,
    modifiers: SubCommandModifiers,
) -> Result<(), HiveLibError> {
    let header_span = Span::current();
    header_span.pb_set_style(&ProgressStyle::default_bar());
    header_span.pb_set_length(1);

    // Respect user's --always-local arg
    hive.force_always_local(always_local)?;

    let header_span_enter = header_span.enter();

    let (tags, names) = on.iter().fold(
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
        .iter()
        .filter(|(name, node)| {
            on.is_empty() || names.contains(name) || node.tags.iter().any(|tag| tags.contains(tag))
        })
        .map(|node| {
            let path = hive.path.clone();
            let span = header_span.clone();

            info!("Resolved {on:?} to include {}", node.0);

            node.achieve_goal(path, span, &goal, no_keys, modifiers)
        })
        .peekable();

    if set.peek().is_none() {
        error!("There are no nodes selected for deployment");
    }

    let futures = futures::stream::iter(set).buffer_unordered(parallel);
    let result: Result<(), _> = futures.collect::<Vec<_>>().await.into_iter().collect();

    std::mem::drop(header_span_enter);
    std::mem::drop(header_span);

    result
}
