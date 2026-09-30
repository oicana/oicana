//! A command line interface for Oicana.
//!
//! Among other things, this CLI can validate and package Oicana templates.

mod checks;
mod compile;
mod fonts;
mod new;
mod pack;
mod target;
mod test;
#[cfg(feature = "self-update")]
mod update;
mod validate;
mod watch;

use crate::compile::compile;
use crate::new::{new, NewArgs, NEW_AFTER_HELP};
use crate::pack::{pack, PackArgs};
use crate::validate::{validate, ValidateArgs};
use crate::watch::WATCH_AFTER_HELP;
use anyhow::Error;
use clap::Parser;
use clap_verbosity::{Verbosity, WarnLevel};
use compile::{CompileArgs, COMPILE_AFTER_HELP};
use console::style;
use log::trace;
use oicana::CompileError;
use pack::PACK_AFTER_HELP;
use std::process::ExitCode;
use test::{test, TestArgs, TEST_AFTER_HELP};
use validate::VALIDATE_AFTER_HELP;
use watch::watch;

fn main() -> ExitCode {
    reset_sigpipe();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&error);
            ExitCode::FAILURE
        }
    }
}

/// Let a closed stdout end the process quietly
///
/// E.g. for `oicana test -a | head -n 1`
#[cfg(unix)]
fn reset_sigpipe() {
    // SAFETY: runs before any other thread exists.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
fn reset_sigpipe() {}

fn run() -> Result<(), Error> {
    let cli = Cli::parse();
    env_logger::Builder::new()
        .filter_level(cli.verbosity.log_level_filter())
        .init();
    trace!("{cli:?}");

    match cli.command {
        Oicana::New(args) => new(args)?,
        Oicana::Compile(args) => compile(args)?,
        Oicana::Watch(args) => watch(args)?,
        Oicana::Validate(validate_args) => validate(validate_args)?,
        Oicana::Pack(package_args) => pack(package_args)?,
        Oicana::Test(test_args) => test(test_args)?,
        #[cfg(feature = "self-update")]
        Oicana::Update => update::update()?,
    }

    Ok(())
}

/// Report a top-level error on stderr.
///
/// Failed compilations already carry Typst's own `error:` label from the
/// rendered diagnostics. Every other error gets the label here, so the CLI
/// consistently marks errors the way Typst does.
fn report(error: &Error) {
    if matches!(
        error.downcast_ref::<CompileError>(),
        Some(CompileError::CompilationFailed(_))
    ) {
        eprintln!("{error:?}");
    } else {
        eprintln!("{} {error:?}", style("error:").red().bold().for_stderr());
    }
}

const VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (comes with typst ",
    env!("TYPST_VERSION"),
    ")"
);

#[derive(Parser, Debug)]
#[clap(name = "oicana", version = VERSION, author)]
#[command(name = "oicana")]
#[command(after_help = AFTER_HELP)]
#[command(about = "PDF templating with Typst", long_about = LONG_ABOUT)]
struct Cli {
    #[command(subcommand)]
    command: Oicana,
    #[command(flatten)]
    verbosity: Verbosity<WarnLevel>,
}

#[derive(Parser, Debug)]
enum Oicana {
    #[clap(about = "Scaffold a new Oicana template", after_help = NEW_AFTER_HELP)]
    New(NewArgs),
    #[clap(about = "Compile oicana templates", after_help = COMPILE_AFTER_HELP)]
    Compile(CompileArgs),
    #[clap(about = "Recompile oicana templates on changes", after_help = WATCH_AFTER_HELP)]
    Watch(CompileArgs),
    #[clap(about = "Validate oicana templates", after_help = VALIDATE_AFTER_HELP)]
    Validate(ValidateArgs),
    #[clap(about = "Package oicana templates", after_help = PACK_AFTER_HELP)]
    Pack(PackArgs),
    #[clap(about = "Test oicana templates", after_help = TEST_AFTER_HELP)]
    Test(TestArgs),
    #[cfg(feature = "self-update")]
    #[clap(about = "Update oicana to the latest release")]
    Update,
}

/// Adds a list of useful links after the normal help text.
#[rustfmt::skip]
const AFTER_HELP: &str = color_print::cstr!("\
<s><u>Resources:</></>
  <s>Website:</>  https://oicana.com/
  <s>Code:</>     https://github.com/oicana/oicana/
");

const LONG_ABOUT: &str = r#"
Oicana is a set of tools and libraries to write document templates
using Typst and create PDFs from these templates out of code.
Oicana templates can define inputs and receive values for
these inputs through libraries for different
programming languages and platforms.
"#;
