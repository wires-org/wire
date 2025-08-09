use miette::{IntoDiagnostic as _, Result, miette};
use std::{
    env,
    fmt::{self, Display, Formatter},
    fs,
    path::Path,
};

use itertools::Itertools;
use proc_macro2::TokenTree;
use syn::{Item, ItemEnum, Meta, MetaList, parse_file};

macro_rules! p {
    ($($tokens: tt)*) => {
        println!("cargo::warning={}", format!($($tokens)*))
    }
}

#[derive(Debug)]
struct DerviedError {
    code: String,
    help: Option<String>,
}

impl Display for DerviedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "## `{code}` {{#{code}}}

{help}",
            code = self.code,
            help = match &self.help {
                Some(help) => format!(
                    "::: tip HELP
{help}
:::"
                ),
                None => "".to_string(),
            }
        )
    }
}

impl TryFrom<MetaList> for DerviedError {
    type Error = miette::Error;

    fn try_from(list: MetaList) -> std::result::Result<Self, Self::Error> {
        if list.path.segments.last().unwrap().ident != "diagnostic" {
            return Err(miette!("Not a diagnostic"));
        }

        let vec: Vec<_> = list.tokens.into_iter().collect();

        // Find `diagnostic(code(x::y::z))`
        let code: Option<String> = if let Some((_, TokenTree::Group(group))) =
            vec.iter().tuple_windows().find(|(ident, group)| {
                matches!(ident, TokenTree::Ident(ident) if ident == "code")
                    && matches!(group, TokenTree::Group(..))
            }) {
            Some(group.stream().to_string().replace(" ", ""))
        } else {
            None
        };

        // Find `diagnostic(help("hi"))`
        let help: Option<String> = if let Some((_, TokenTree::Group(group))) =
            vec.iter().tuple_windows().find(|(ident, group)| {
                matches!(ident, TokenTree::Ident(ident) if ident == "help")
                    && matches!(group, TokenTree::Group(..))
            }) {
            Some(group.stream().to_string())
        } else {
            None
        };

        if let Some(code) = code {
            return Ok(DerviedError { code, help });
        }

        Err(miette!("Had no code."))
    }
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=src/hive/steps/keys.rs");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").into_diagnostic()?;
    let out_dir = if let Ok(path) = env::var("DIAGNOSTICS_MD_OUTPUT") {
        path
    } else {
        return Ok(());
    };

    p!("{out_dir}");

    let src_path = Path::new(&manifest_dir).join("src/hive/steps/keys.rs");
    let src = fs::read_to_string(&src_path).into_diagnostic()?;

    let syntax_tree = parse_file(&src).into_diagnostic()?;
    let mut entries: Vec<DerviedError> = Vec::new();

    for item in &syntax_tree.items {
        if let Item::Enum(ItemEnum { variants, .. }) = item {
            for variant in variants {
                for attribute in variant.attrs.clone() {
                    // p!("{:?}", attribute);

                    if let Meta::List(list) = attribute.meta {
                        if let Ok(entry) = list.try_into() {
                            entries.push(entry);
                        }
                    }
                }
            }
        }
    }

    fs::write(
        Path::new(&out_dir).join("DIAGNOSTICS.md"),
        entries.iter().map(|x| x.to_string()).join("\n\n"),
    )
    .into_diagnostic()?;

    Ok(())
}
