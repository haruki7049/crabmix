//! # xtask
//!
//! A cargo-xtask runner for the `crabmix` workspace.
//!
//! This binary provides a convenient way to run common development tasks
//! (build, check, clippy, test, doc) across the entire workspace in both
//! debug and release profiles. It is invoked via `cargo xtask <action>`.
//!
//! ## Available actions
//!
//! | Action    | Description                                          |
//! |-----------|------------------------------------------------------|
//! | `all`     | Runs build, check, clippy, test, and doc (default).  |
//! | `build`   | Builds the workspace in debug and release mode.      |
//! | `check`   | Checks the workspace in debug and release mode.      |
//! | `clippy`  | Runs Clippy on the workspace in debug and release.   |
//! | `test`    | Runs tests in debug and release mode.                |
//! | `doc`     | Generates documentation in debug and release mode.   |

use clap::{Parser, Subcommand};
use std::process::Command;
use std::sync::LazyLock;
use thiserror::Error;

type Result = std::result::Result<(), Box<dyn std::error::Error>>;

fn main() -> Result {
    tracing_subscriber::fmt::init();

    tracing::debug!("Parsing CLI arguments...");
    let args = CLIArgs::parse();
    tracing::debug!("Parsed CLI arguments.");

    match &args.action {
        Action::All => all()?,
        Action::Build => build()?,
        Action::Check => check()?,
        Action::Clippy => clippy()?,
        Action::Test => test()?,
        Action::Doc => doc()?,
    }

    Ok(())
}

/// Command-line arguments for the xtask runner.
#[derive(Parser)]
struct CLIArgs {
    /// The development action to perform. Defaults to `All`.
    #[clap(default_value_t = Action::All)]
    action: Action,
}

/// A development action that can be run across the workspace.
#[derive(Subcommand, Clone)]
enum Action {
    /// Run all actions: build, check, clippy, test, and doc.
    All,
    /// Build the workspace in debug and release mode.
    Build,
    /// Check the workspace in debug and release mode.
    Check,
    /// Run Clippy on the workspace in debug and release mode.
    Clippy,
    /// Run tests in debug and release mode.
    Test,
    /// Generate documentation in debug and release mode.
    Doc,
}

impl std::fmt::Display for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Build => write!(f, "build"),
            Self::Check => write!(f, "check"),
            Self::Clippy => write!(f, "clippy"),
            Self::Test => write!(f, "test"),
            Self::Doc => write!(f, "doc"),
        }
    }
}

impl std::str::FromStr for Action {
    type Err = ActionParseError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            "build" => Ok(Self::Build),
            "check" => Ok(Self::Check),
            "clippy" => Ok(Self::Clippy),
            "test" => Ok(Self::Test),
            "doc" => Ok(Self::Doc),
            v => Err(Self::Err::ParseError(v.to_string())),
        }
    }
}

/// Errors that can occur when parsing an [`Action`] from a string.
#[derive(Debug, Error)]
enum ActionParseError {
    /// The input string did not match any known action.
    ParseError(String),
}

impl std::fmt::Display for ActionParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ParseError(s) => write!(f, "ParseError by \"{}\"", s),
        }
    }
}

static CARGO: LazyLock<String> =
    LazyLock::new(|| std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()));

#[tracing::instrument]
fn all() -> Result {
    tracing::info!("Running...");
    build()?;
    check()?;
    clippy()?;
    test()?;
    doc()?;
    tracing::info!("Finished.");

    Ok(())
}

#[tracing::instrument]
fn build() -> Result {
    tracing::info!("Starting...");

    build_debug()?;
    build_release()?;

    tracing::info!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn build_debug() -> Result {
    tracing::debug!("Running...");

    let mut build_command = Command::new(CARGO.as_str());
    build_command.arg("build");
    build_command.arg("--workspace");

    let exit_status = build_command.spawn()?.wait()?;

    if !exit_status.success() {
        panic!("cargo build --workspace is failed");
    }

    tracing::debug!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn build_release() -> Result {
    tracing::debug!("Running...");

    let mut build_release_command = Command::new(CARGO.as_str());
    build_release_command.arg("build");
    build_release_command.arg("--release");
    build_release_command.arg("--workspace");

    let exit_status = build_release_command.spawn()?.wait()?;

    if !exit_status.success() {
        panic!("cargo build --release --workspace is failed");
    }

    tracing::debug!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn check() -> Result {
    tracing::info!("Starting...");

    check_debug()?;
    check_release()?;

    tracing::info!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn check_debug() -> Result {
    tracing::debug!("Running...");

    let mut check_command = Command::new(CARGO.as_str());
    check_command.arg("check");
    check_command.arg("--workspace");

    let exit_status = check_command.spawn()?.wait()?;

    if !exit_status.success() {
        panic!("cargo check --workspace is failed");
    }

    tracing::debug!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn check_release() -> Result {
    tracing::debug!("Running...");

    let mut check_release_command = Command::new(CARGO.as_str());
    check_release_command.arg("check");
    check_release_command.arg("--release");
    check_release_command.arg("--workspace");

    let exit_status = check_release_command.spawn()?.wait()?;

    if !exit_status.success() {
        panic!("cargo check --release --workspace is failed");
    }

    tracing::debug!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn clippy() -> Result {
    tracing::info!("Starting...");

    clippy_debug()?;
    clippy_release()?;

    // cargo clippy --release --workspace
    {}

    tracing::info!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn clippy_debug() -> Result {
    tracing::debug!("Running...");

    let mut clippy_command = Command::new(CARGO.as_str());
    clippy_command.arg("clippy");
    clippy_command.arg("--workspace");

    let exit_status = clippy_command.spawn()?.wait()?;

    if !exit_status.success() {
        panic!("cargo clippy --workspace is failed");
    }

    tracing::debug!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn clippy_release() -> Result {
    tracing::debug!("Running...");

    let mut clippy_release_command = Command::new(CARGO.as_str());
    clippy_release_command.arg("clippy");
    clippy_release_command.arg("--release");
    clippy_release_command.arg("--workspace");

    let exit_status = clippy_release_command.spawn()?.wait()?;

    if !exit_status.success() {
        panic!("cargo clippy --release --workspace is failed");
    }

    tracing::debug!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn test() -> Result {
    tracing::info!("Starting...");

    test_debug()?;
    test_release()?;

    // cargo test --release --workspace
    {
        let mut test_release_command = Command::new(CARGO.as_str());
        test_release_command.arg("test");
        test_release_command.arg("--release");
        test_release_command.arg("--workspace");

        let exit_status = test_release_command.spawn()?.wait()?;

        if !exit_status.success() {
            panic!("cargo test --release --workspace is failed");
        }
    }

    tracing::info!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn test_debug() -> Result {
    tracing::debug!("Running...");

    let mut test_command = Command::new(CARGO.as_str());
    test_command.arg("test");
    test_command.arg("--workspace");

    let exit_status = test_command.spawn()?.wait()?;

    if !exit_status.success() {
        panic!("cargo test --workspace is failed");
    }

    tracing::debug!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn test_release() -> Result {
    tracing::debug!("Running...");

    let mut test_release_command = Command::new(CARGO.as_str());
    test_release_command.arg("test");
    test_release_command.arg("--release");
    test_release_command.arg("--workspace");

    let exit_status = test_release_command.spawn()?.wait()?;

    if !exit_status.success() {
        panic!("cargo test --release --workspace is failed");
    }

    tracing::debug!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn doc() -> Result {
    tracing::info!("Running...");

    doc_debug()?;
    doc_release()?;

    tracing::info!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn doc_debug() -> Result {
    tracing::debug!("Running...");

    let mut doc_command = Command::new(CARGO.as_str());
    doc_command.arg("doc");
    doc_command.arg("--workspace");

    let exit_status = doc_command.spawn()?.wait()?;

    if !exit_status.success() {
        panic!("cargo doc --workspace is failed");
    }

    tracing::debug!("Finished.");
    Ok(())
}

#[tracing::instrument]
fn doc_release() -> Result {
    tracing::debug!("Running...");

    let mut doc_release_command = Command::new(CARGO.as_str());
    doc_release_command.arg("doc");
    doc_release_command.arg("--release");
    doc_release_command.arg("--workspace");

    let exit_status = doc_release_command.spawn()?.wait()?;

    if !exit_status.success() {
        panic!("cargo doc --release --workspace is failed");
    }

    tracing::debug!("Finished.");
    Ok(())
}
