use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::MenderError;

#[derive(Parser, Debug)]
#[command(name = "mender", version, about = "Failure triage and repair planner")]
pub struct Cli {
    /// Project root override
    #[arg(long, global = true)]
    pub repo: Option<PathBuf>,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn resolve_repo(&self) -> Result<PathBuf, MenderError> {
        if let Some(ref repo) = self.repo {
            return Ok(repo.clone());
        }
        if let Ok(output) = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                return Ok(PathBuf::from(path));
            }
        }
        std::env::current_dir().map_err(MenderError::Io)
    }

    pub fn is_json(&self) -> bool {
        matches!(self.format, OutputFormat::Json)
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum OutputFormat {
    Json,
    Text,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Analyze build/test output and triage failures
    Triage {
        /// Read error output from a file instead of stdin
        #[arg(long)]
        file: Option<PathBuf>,
        /// Read error output from a command (runs it)
        #[arg(long, num_args = 1..)]
        run: Option<Vec<String>>,
    },
    /// Show supported error pattern categories
    Patterns,
}
