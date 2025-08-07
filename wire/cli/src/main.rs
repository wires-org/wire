#![deny(clippy::pedantic)]
#![allow(clippy::missing_panics_doc)]
use crate::cli::Cli;
use crate::cli::ToSubCommandModifiers;
use clap::CommandFactory;
use clap::Parser;
use clap_complete::generate;
use clap_verbosity_flag::{Verbosity, WarnLevel};
use indicatif::style::ProgressStyle;
use lib::hive::Hive;
use miette::IntoDiagnostic;
use miette::Result;
use tracing::warn;
use tracing_indicatif::IndicatifLayer;
use tracing_log::AsTrace;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, Registry};

#[macro_use]
extern crate enum_display_derive;

mod apply;
mod cli;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[tokio::main]
async fn main() -> Result<()> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    let args = Cli::parse();

    let modifiers = args.to_subcommand_modifiers();
    setup_logging(args.no_progress, &args.verbose);

    if args.markdown_help {
        clap_markdown::print_help_markdown::<Cli>();
        return Ok(());
    }

    match args.command {
        cli::Commands::Apply(apply_args) => {
            let mut hive = Hive::new_from_path(args.path.as_path(), modifiers).await?;
            apply::apply(&mut hive, apply_args, args.path, modifiers).await?;
        }
        cli::Commands::Inspect { online: _, json } => println!("{}", {
            let hive = Hive::new_from_path(args.path.as_path(), modifiers).await?;
            if json {
                serde_json::to_string(&hive).into_diagnostic()?
            } else {
                warn!("use --json to output something scripting suitable");
                format!("{hive:#?}")
            }
        }),
        cli::Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            let name = cmd.clone();
            generate(shell, &mut cmd, name.get_name(), &mut std::io::stdout());
        }
    }

    Ok(())
}

pub fn setup_logging(no_progress: bool, verbosity: &Verbosity<WarnLevel>) {
    let layer = tracing_subscriber::fmt::layer::<Registry>().without_time();
    let filter = verbosity.log_level_filter().as_trace();
    let registry = tracing_subscriber::registry();

    if no_progress {
        let layer = layer.with_filter(filter);

        registry.with(layer).init();
    } else {
        let indicatif_layer = IndicatifLayer::new().with_progress_style(
            ProgressStyle::with_template(
                "{span_child_prefix}[{spinner}] {span_name}{{{span_fields}}} {wide_msg}",
            )
            .expect("Failed to create progress style"),
        );

        let layer = layer
            .with_writer(indicatif_layer.get_stderr_writer())
            .with_filter(filter);

        registry.with(layer).with(indicatif_layer).init();
    }
}
