use std::sync::LazyLock;
use regex::Regex;
use serde::Serialize;

/// A single parsed error from build/test output.
#[derive(Debug, Clone, Serialize)]
pub struct ParsedError {
    pub file: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub message: String,
    pub error_type: ErrorType,
    pub severity: Severity,
    pub raw_line: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    CompileError,
    TypeError,
    ImportError,
    TestFailure,
    LinkError,
    Warning,
    Panic,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Parse raw build/test output into structured errors.
pub fn parse_errors(output: &str) -> Vec<ParsedError> {
    let mut errors = Vec::new();

    for line in output.lines() {
        if let Some(err) = try_parse_rust_error(line) {
            errors.push(err);
        } else if let Some(err) = try_parse_rust_test_failure(line) {
            errors.push(err);
        } else if let Some(err) = try_parse_typescript_error(line) {
            errors.push(err);
        } else if let Some(err) = try_parse_python_error(line) {
            errors.push(err);
        } else if let Some(err) = try_parse_go_error(line) {
            errors.push(err);
        } else if let Some(err) = try_parse_generic_error(line) {
            errors.push(err);
        }
    }

    errors
}

// --- Rust ---

fn try_parse_rust_error(line: &str) -> Option<ParsedError> {
    // error[E0432]: unresolved import `crate::foo`
    //  --> src/main.rs:3:5
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^error(?:\[E\d+\])?: (.+)$").unwrap());
    static LOC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+--> (.+):(\d+):(\d+)$").unwrap());
    let re = &*RE;
    let loc_re = &*LOC_RE;

    if let Some(cap) = re.captures(line) {
        let message = cap[1].to_string();
        let error_type = classify_rust_error(&message);

        return Some(ParsedError {
            file: None, // Will be filled from the next line in clustering
            line: None,
            column: None,
            message,
            error_type,
            severity: Severity::Error,
            raw_line: line.to_string(),
        });
    }

    // Try location line
    if let Some(cap) = loc_re.captures(line) {
        return Some(ParsedError {
            file: Some(cap[1].to_string()),
            line: cap[2].parse().ok(),
            column: cap[3].parse().ok(),
            message: String::new(), // Will be merged with previous error
            error_type: ErrorType::CompileError,
            severity: Severity::Info, // Location marker, not standalone
            raw_line: line.to_string(),
        });
    }

    // warning: unused variable
    static WARN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^warning(?:\[[\w]+\])?: (.+)$").unwrap());
    let warn_re = &*WARN_RE;
    if let Some(cap) = warn_re.captures(line) {
        return Some(ParsedError {
            file: None,
            line: None,
            column: None,
            message: cap[1].to_string(),
            error_type: ErrorType::Warning,
            severity: Severity::Warning,
            raw_line: line.to_string(),
        });
    }

    None
}

fn try_parse_rust_test_failure(line: &str) -> Option<ParsedError> {
    // test my_test ... FAILED
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^test (.+) \.\.\. FAILED$").unwrap());
    let re = &*RE;
    if let Some(cap) = re.captures(line) {
        return Some(ParsedError {
            file: None,
            line: None,
            column: None,
            message: format!("Test failed: {}", &cap[1]),
            error_type: ErrorType::TestFailure,
            severity: Severity::Error,
            raw_line: line.to_string(),
        });
    }

    // thread 'test_name' panicked at 'message', file:line:col
    static PANIC_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"thread '(.+?)' .* panicked at (.+)").unwrap());
    let panic_re = &*PANIC_RE;
    if let Some(cap) = panic_re.captures(line) {
        return Some(ParsedError {
            file: None,
            line: None,
            column: None,
            message: format!("Panic in {}: {}", &cap[1], &cap[2]),
            error_type: ErrorType::Panic,
            severity: Severity::Error,
            raw_line: line.to_string(),
        });
    }

    None
}

fn classify_rust_error(message: &str) -> ErrorType {
    if message.contains("unresolved import") || message.contains("could not find") {
        ErrorType::ImportError
    } else if message.contains("mismatched types") || message.contains("expected") {
        ErrorType::TypeError
    } else if message.contains("cannot find") || message.contains("not found in scope") {
        ErrorType::ImportError
    } else if message.contains("linking with") {
        ErrorType::LinkError
    } else {
        ErrorType::CompileError
    }
}

// --- TypeScript ---

fn try_parse_typescript_error(line: &str) -> Option<ParsedError> {
    // src/foo.ts(10,5): error TS2304: Cannot find name 'foo'.
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(.+?)\((\d+),(\d+)\): (error|warning) (TS\d+): (.+)$").unwrap());
    let re = &*RE;
    if let Some(cap) = re.captures(line) {
        let severity = if &cap[4] == "error" { Severity::Error } else { Severity::Warning };
        return Some(ParsedError {
            file: Some(cap[1].to_string()),
            line: cap[2].parse().ok(),
            column: cap[3].parse().ok(),
            message: format!("{}: {}", &cap[5], &cap[6]),
            error_type: ErrorType::TypeError,
            severity,
            raw_line: line.to_string(),
        });
    }
    None
}

// --- Python ---

fn try_parse_python_error(line: &str) -> Option<ParsedError> {
    // File "foo.py", line 10, in bar
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"^\s*File "(.+?)", line (\d+)"#).unwrap());
    let re = &*RE;
    if let Some(cap) = re.captures(line) {
        return Some(ParsedError {
            file: Some(cap[1].to_string()),
            line: cap[2].parse().ok(),
            column: None,
            message: String::new(), // The actual error is on the next line
            error_type: ErrorType::CompileError,
            severity: Severity::Info,
            raw_line: line.to_string(),
        });
    }

    // ImportError: No module named 'foo'
    static IMPORT_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(Import|Module)Error: (.+)$").unwrap());
    let import_re = &*IMPORT_RE;
    if let Some(cap) = import_re.captures(line) {
        return Some(ParsedError {
            file: None,
            line: None,
            column: None,
            message: cap[2].to_string(),
            error_type: ErrorType::ImportError,
            severity: Severity::Error,
            raw_line: line.to_string(),
        });
    }

    // TypeError, ValueError, etc.
    static ERR_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\w+Error): (.+)$").unwrap());
    let err_re = &*ERR_RE;
    if let Some(cap) = err_re.captures(line) {
        return Some(ParsedError {
            file: None,
            line: None,
            column: None,
            message: format!("{}: {}", &cap[1], &cap[2]),
            error_type: ErrorType::CompileError,
            severity: Severity::Error,
            raw_line: line.to_string(),
        });
    }

    None
}

// --- Go ---

fn try_parse_go_error(line: &str) -> Option<ParsedError> {
    // ./main.go:10:5: undefined: foo
    static RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(.+?\.go):(\d+):(\d+): (.+)$").unwrap());
    let re = &*RE;
    if let Some(cap) = re.captures(line) {
        return Some(ParsedError {
            file: Some(cap[1].to_string()),
            line: cap[2].parse().ok(),
            column: cap[3].parse().ok(),
            message: cap[4].to_string(),
            error_type: ErrorType::CompileError,
            severity: Severity::Error,
            raw_line: line.to_string(),
        });
    }
    None
}

// --- Generic ---

fn try_parse_generic_error(line: &str) -> Option<ParsedError> {
    let lower = line.to_lowercase();
    if lower.contains("error:") || lower.contains("fatal:") {
        Some(ParsedError {
            file: None,
            line: None,
            column: None,
            message: line.trim().to_string(),
            error_type: ErrorType::Unknown,
            severity: Severity::Error,
            raw_line: line.to_string(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rust_error() {
        let output = "error[E0432]: unresolved import `crate::foo`\n --> src/main.rs:3:5\n";
        let errors = parse_errors(output);
        assert_eq!(errors.len(), 2);
        assert_eq!(errors[0].error_type, ErrorType::ImportError);
    }

    #[test]
    fn parse_rust_test_failure() {
        let output = "test my_module::tests::it_works ... FAILED\n";
        let errors = parse_errors(output);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_type, ErrorType::TestFailure);
    }

    #[test]
    fn parse_typescript_error() {
        let output = "src/foo.ts(10,5): error TS2304: Cannot find name 'bar'.\n";
        let errors = parse_errors(output);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file, Some("src/foo.ts".to_string()));
    }

    #[test]
    fn parse_python_import_error() {
        let output = "ImportError: No module named 'flask'\n";
        let errors = parse_errors(output);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].error_type, ErrorType::ImportError);
    }

    #[test]
    fn parse_go_error() {
        let output = "./main.go:10:5: undefined: myFunc\n";
        let errors = parse_errors(output);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file, Some("./main.go".to_string()));
    }

    #[test]
    fn parse_empty_input() {
        assert!(parse_errors("").is_empty());
        assert!(parse_errors("\n\n\n").is_empty());
    }
}
