#![cfg(feature = "cli")]

use anyhow::{bail, Context};
use chrono::{Local, NaiveDateTime};
use clap::{Parser, Subcommand};
use orgish::{Document, Format, Keyword};
use orgish_tools::{mark_nodes_done, refile_to_file, CompletedNode};
use std::io::{self, BufRead};

fn main() -> Result<(), anyhow::Error> {
    // Parse the CLI options
    let opts = Options::parse();
    let format = match opts.format.as_str() {
        "markdown" | "md" => Format::Markdown,
        "org" => Format::Org,
        _ => bail!("invalid format, expected markdown or org"),
    };

    // Read from stdin into a string until EOF
    let mut input = Vec::new();
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        input.push(line);
    }
    let input = input.join("\n");

    // Parse that as a document (it *should* only be one heading, but we might do an en-masse
    // refile)
    let document = Document::<GenericKeyword>::from_str(&input, format)
        .with_context(|| "failed to parse stdin as orgish document")?;
    // Make sure there's no root text (that would be a bad selection)
    if document.root.body.is_some() {
        bail!("invalid selection, expected no root contents")
    }
    let fragment = document.root.into_children();

    match opts.command {
        Command::MarkDone {
            target,
            keyword,
            repeating_keyword,
            no_last_repeat,
            last_repeat,
        } => {
            let keyword = GenericKeyword { keyword };
            let repeating_keyword = GenericKeyword {
                keyword: repeating_keyword,
            };

            let now = Local::now();
            let parsed_nodes = mark_nodes_done(
                fragment,
                repeating_keyword,
                keyword,
                if no_last_repeat {
                    None
                } else {
                    Some(last_repeat.unwrap_or(now.naive_local()))
                },
            );

            let mut to_refile = Vec::new();
            for node in parsed_nodes {
                match node {
                    CompletedNode::Done(node) => {
                        if target.is_some() {
                            to_refile.push(node)
                        } else {
                            println!("{}", node.into_string(format));
                        }
                    }
                    CompletedNode::Repeating {
                        completed,
                        repeating,
                    } => {
                        // Regardless, the repeating node should go into the filter, but the
                        // completed node will never. It might be refiled though
                        println!("{}", repeating.into_string(format));
                        if target.is_some() {
                            to_refile.push(completed);
                        }
                    }
                }
            }

            if let Some(target) = target {
                refile_to_file(to_refile, target, format)?;
            }
        }
    }

    Ok(())
}

/// Performs a variety of operations on Orgish nodes to add support for Org mode-style
/// functionality to any editor and workflow
#[derive(Parser, Debug)]
struct Options {
    #[command(subcommand)]
    command: Command,
    /// The format to parse the input document into (`markdown`|`org`)
    #[arg(short, long)]
    format: String,
}
#[derive(Subcommand, Debug)]
enum Command {
    /// Marks the nodes as `DONE`, progressing any repeating timestamps
    MarkDone {
        /// Where to put the non-repeating nodes which are marked as done (repeating nodes
        /// will be returned through stdout in their new form). If not set, non-repeating
        /// nodes will be returned with a `DONE` keyword
        #[arg(short, long)]
        target: Option<String>,
        /// An alternative keyword to use for the `DONE` state of nodes that don't repeat
        #[arg(short, long, default_value = "DONE")]
        keyword: String,
        /// An alternative keyword to use for nodes that don't repeat
        #[arg(long, default_value = "TODO")]
        repeating_keyword: String,
        /// Disable setting the `LAST_REPEAT` property
        #[arg(long)]
        no_last_repeat: bool,
        /// Set a custom completion time for `LAST_REPEAT`
        #[arg(long)]
        last_repeat: Option<NaiveDateTime>,
    },
}

/// A generic keyword detection system for Orgish that calls any completely uppercase word a valid
/// keyword. This should be used for command-line systems only, where the flexibility of an `enum`
/// is not available at compile-time.
#[derive(Clone)]
struct GenericKeyword {
    keyword: String,
}
impl Keyword for GenericKeyword {
    fn from_str(keyword: &str) -> Option<Self> {
        if keyword.chars().all(|c| c.is_uppercase()) {
            Some(Self {
                keyword: keyword.to_string(),
            })
        } else {
            None
        }
    }

    fn into_string(self) -> String {
        self.keyword
    }

    fn other(keyword: String) -> Self {
        Self { keyword }
    }
}
