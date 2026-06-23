mod cli;
mod parse;
mod report;
mod triage;

use std::io::Read;
use clap::Parser;
use cli::{Cli, Command};

fn main() {
    let cli = Cli::parse();
    let result = run(&cli);
    match result {
        Ok(()) => {}
        Err(e) => {
            let code = e.exit_code();
            if cli.is_json() {
                let err_json = serde_json::json!({
                    "ok": false,
                    "error": {
                        "code": e.error_code(),
                        "message": e.to_string(),
                    }
                });
                eprintln!("{}", serde_json::to_string_pretty(&err_json).unwrap_or_else(|_| format!("{{\"ok\":false,\"error\":{{\"message\":\"{e}\"}}}}")));
            } else {
                eprintln!("error: {e}");
            }
            std::process::exit(code);
        }
    }
}

fn run(cli: &Cli) -> Result<(), MenderError> {
    match &cli.command {
        Command::Triage { file, run } => {
            let output = get_error_output(file.as_deref(), run.as_deref())?;
            let errors = parse::parse_errors(&output);
            let result = triage::triage(&errors);
            report::print_triage(&result, cli.is_json())
        }
        Command::Patterns => {
            report::print_patterns(cli.is_json())
        }
    }
}

fn get_error_output(
    file: Option<&std::path::Path>,
    run: Option<&[String]>,
) -> Result<String, MenderError> {
    if let Some(path) = file {
        std::fs::read_to_string(path).map_err(MenderError::Io)
    } else if let Some(cmd_parts) = run {
        if cmd_parts.is_empty() {
            return Err(MenderError::Validation("No command provided".into()));
        }
        let output = std::process::Command::new(&cmd_parts[0])
            .args(&cmd_parts[1..])
            .output()
            .map_err(MenderError::Io)?;

        let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
        combined.push('\n');
        combined.push_str(&String::from_utf8_lossy(&output.stderr));
        Ok(combined)
    } else {
        // Read from stdin
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input).map_err(MenderError::Io)?;
        Ok(input)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MenderError {
    #[error("{0}")]
    Validation(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl MenderError {
    pub fn exit_code(&self) -> i32 {
        match self {
            MenderError::Validation(_) => 1,
            MenderError::Io(_) => 2,
            MenderError::Json(_) => 1,
        }
    }

    pub fn error_code(&self) -> &'static str {
        match self {
            MenderError::Validation(_) => "validation_error",
            MenderError::Io(_) => "io_error",
            MenderError::Json(_) => "json_error",
        }
    }
}
