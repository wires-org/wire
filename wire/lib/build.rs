use miette::{Context, IntoDiagnostic as _, Result, miette};
use std::{
    env,
    fmt::{self, Display, Formatter},
    fs::{self, Metadata},
    path::Path,
};

use itertools::Itertools;
use proc_macro2::TokenTree;
use syn::{Expr, Item, ItemEnum, Lit, Meta, MetaList, MetaNameValue, parse_file};

macro_rules! p {
    ($($tokens: tt)*) => {
        println!("cargo::warning={}", format!($($tokens)*))
    }
}

#[derive(Debug)]
struct DerviedError {
    code: Option<String>,
    help: Option<String>,
    message: Option<String>,
    doc_string: String,
}

impl Display for DerviedError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "## `{code}` {{#{code}}}

{doc}
{message}
{help}",
            doc = self.doc_string,
            code = self.code.as_ref().unwrap(),
            help = match &self.help {
                Some(help) => format!(
                    "
::: tip HELP
{help}
:::"
                ),
                None => "".to_string(),
            },
            message = match &self.message {
                Some(message) => format!(
                    "
```txt [message]
{message}
```"
                ),
                None => "".to_string(),
            }
        )
    }
}

impl DerviedError {
    fn get_error(&mut self, list: &MetaList) -> Result<(), miette::Error> {
        if list.path.segments.last().unwrap().ident != "error" {
            return Err(miette!("Not an error"));
        }

        self.message = Some(
            list.tokens
                .clone()
                .into_iter()
                .filter(|tok| matches!(tok, TokenTree::Literal(tok) if tok.to_string().starts_with("\"")))
                .map(|tok| tok.to_string())
                .join(""),
        );

        Err(miette!("No error msg found"))
    }

    fn update_diagnostic(&mut self, list: &MetaList) -> Result<(), miette::Error> {
        if list.path.segments.last().unwrap().ident != "diagnostic" {
            return Err(miette!("Not a diagnostic"));
        }

        let vec: Vec<_> = list.tokens.clone().into_iter().collect();

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
            self.code = Some(code);
            self.help = help;
            return Ok(());
        }

        Err(miette!("Had no code."))
    }

    fn update_from_list(&mut self, list: MetaList) {
        let _ = self.get_error(&list);
        let _ = self.update_diagnostic(&list);
    }

    fn update_from_namevalue(&mut self, list: MetaNameValue) -> Result<(), miette::Error> {
        if list.path.segments.last().unwrap().ident != "doc" {
            return Err(miette!("Not a doc string"));
        }

        if let Expr::Lit(lit) = list.value {
            if let Lit::Str(str) = lit.lit {
                self.doc_string
                    .push_str(&format!("{}\n\n", &str.value()[1..]));
            }
        }

        Ok(())
    }
}

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=src/errors.rs");

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").into_diagnostic()?;
    let md_out_dir = if let Ok(path) = env::var("DIAGNOSTICS_MD_OUTPUT") {
        path
    } else {
        return Ok(());
    };

    let src_path = Path::new(&manifest_dir).join("src/errors.rs");
    let src = fs::read_to_string(&src_path)
        .into_diagnostic()
        .wrap_err("reading errors.rs")?;

    let syntax_tree = parse_file(&src)
        .into_diagnostic()
        .wrap_err("parsing errors.rs")?;
    let mut entries: Vec<DerviedError> = Vec::new();

    for item in &syntax_tree.items {
        if let Item::Enum(ItemEnum { variants, .. }) = item {
            for variant in variants {
                let mut entry = DerviedError {
                    code: None,
                    help: None,
                    message: None,
                    doc_string: String::new(),
                };

                for attribute in variant.attrs.clone() {
                    match attribute.meta {
                        Meta::List(list) => {
                            entry.update_from_list(list);
                        }
                        Meta::NameValue(nv) => {
                            let _ = entry.update_from_namevalue(nv);
                        }
                        _ => {}
                    }
                }

                if entry.code.is_some() {
                    entries.push(entry);
                }
            }
        }
    }

    fs::create_dir_all(Path::new(&md_out_dir))
        .into_diagnostic()
        .wrap_err("creating target directory")?;
    fs::write(
        Path::new(&md_out_dir).join("DIAGNOSTICS.md"),
        entries.iter().map(|x| x.to_string()).join("\n\n"),
    )
    .into_diagnostic()
    .wrap_err("writing DIAGNOSTICS.md")?;

    p!(
        "wrote to {:?}",
        Path::new(&md_out_dir).join("DIAGNOSTICS.md")
    );

    Ok(())
}
