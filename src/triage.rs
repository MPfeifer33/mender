use std::collections::HashMap;
use serde::Serialize;

use crate::parse::{ErrorType, ParsedError, Severity};

/// A cluster of related errors (likely same root cause).
#[derive(Debug, Serialize)]
pub struct ErrorCluster {
    pub root_cause: String,
    pub error_type: ErrorType,
    pub severity: Severity,
    pub file: Option<String>,
    pub count: usize,
    pub errors: Vec<ClusterEntry>,
    pub suggested_fix: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ClusterEntry {
    pub message: String,
    pub file: Option<String>,
    pub line: Option<usize>,
}

/// Triage result with clusters and a suggested fix order.
#[derive(Debug, Serialize)]
pub struct TriageResult {
    pub clusters: Vec<ErrorCluster>,
    pub fix_order: Vec<String>,
    pub total_errors: usize,
    pub total_warnings: usize,
}

/// Cluster errors by root cause and suggest fix order.
pub fn triage(errors: &[ParsedError]) -> TriageResult {
    // Merge location markers with their preceding error
    let merged = merge_locations(errors);

    // Group by likely root cause
    let mut clusters = cluster_errors(&merged);

    // Sort clusters: errors first, then by count descending
    clusters.sort_by(|a, b| {
        a.severity.cmp(&b.severity)
            .then(b.count.cmp(&a.count))
    });

    // Generate fix order
    let fix_order = suggest_fix_order(&clusters);

    let total_errors = merged.iter().filter(|e| e.severity == Severity::Error).count();
    let total_warnings = merged.iter().filter(|e| e.severity == Severity::Warning).count();

    TriageResult {
        clusters,
        fix_order,
        total_errors,
        total_warnings,
    }
}

fn merge_locations(errors: &[ParsedError]) -> Vec<ParsedError> {
    let mut merged = Vec::new();
    let mut i = 0;

    while i < errors.len() {
        let err = &errors[i];

        // If this is a real error and next is a location marker, merge them
        if err.severity != Severity::Info && i + 1 < errors.len() {
            let next = &errors[i + 1];
            if next.severity == Severity::Info && next.file.is_some() && next.message.is_empty() {
                merged.push(ParsedError {
                    file: next.file.clone(),
                    line: next.line,
                    column: next.column,
                    message: err.message.clone(),
                    error_type: err.error_type,
                    severity: err.severity,
                    raw_line: err.raw_line.clone(),
                });
                i += 2;
                continue;
            }
        }

        // Skip standalone location markers
        if err.severity == Severity::Info && err.message.is_empty() {
            i += 1;
            continue;
        }

        merged.push(err.clone());
        i += 1;
    }

    merged
}

fn cluster_errors(errors: &[ParsedError]) -> Vec<ErrorCluster> {
    let mut groups: HashMap<String, Vec<&ParsedError>> = HashMap::new();

    for err in errors {
        let key = cluster_key(err);
        groups.entry(key).or_default().push(err);
    }

    groups.into_iter().map(|(key, errs)| {
        let first = errs[0];
        let file = errs.iter()
            .find_map(|e| e.file.clone());
        let suggested_fix = suggest_fix(first.error_type, &first.message);

        ErrorCluster {
            root_cause: key,
            error_type: first.error_type,
            severity: first.severity,
            file,
            count: errs.len(),
            errors: errs.iter().map(|e| ClusterEntry {
                message: e.message.clone(),
                file: e.file.clone(),
                line: e.line,
            }).collect(),
            suggested_fix,
        }
    }).collect()
}

fn cluster_key(err: &ParsedError) -> String {
    match err.error_type {
        ErrorType::ImportError => format!("import: {}", normalize_message(&err.message)),
        ErrorType::TypeError => format!("type: {}", normalize_message(&err.message)),
        ErrorType::TestFailure => format!("test: {}", &err.message),
        ErrorType::Panic => "panic".to_string(),
        ErrorType::LinkError => "link".to_string(),
        ErrorType::Warning => format!("warn: {}", normalize_message(&err.message)),
        ErrorType::CompileError => {
            if let Some(ref file) = err.file {
                format!("compile: {}", file)
            } else {
                format!("compile: {}", normalize_message(&err.message))
            }
        }
        ErrorType::Unknown => format!("unknown: {}", normalize_message(&err.message)),
    }
}

fn normalize_message(msg: &str) -> String {
    // Reduce to first meaningful clause for clustering
    msg.split('\n').next().unwrap_or(msg)
        .chars().take(60).collect()
}

fn suggest_fix(error_type: ErrorType, message: &str) -> Option<String> {
    match error_type {
        ErrorType::ImportError => {
            if message.contains("unresolved import") {
                Some("Check module path, ensure the target module exists and is declared with `mod`".into())
            } else if message.contains("No module named") {
                Some("Install the missing package or check your virtual environment".into())
            } else {
                Some("Verify import path and ensure dependency is listed in manifest".into())
            }
        }
        ErrorType::TypeError => {
            Some("Check type annotations and function signatures at the reported location".into())
        }
        ErrorType::TestFailure => {
            Some("Run the failing test in isolation to reproduce, check assertions".into())
        }
        ErrorType::LinkError => {
            Some("Check for missing native libraries or incompatible dependency versions".into())
        }
        ErrorType::Panic => {
            Some("Check the panic message for assertion failures or unwrap calls on None/Err".into())
        }
        _ => None,
    }
}

fn suggest_fix_order(clusters: &[ErrorCluster]) -> Vec<String> {
    // Fix order: imports first (unblock compilation), then types, then tests
    let priority = |c: &ErrorCluster| -> usize {
        match c.error_type {
            ErrorType::ImportError => 0,
            ErrorType::LinkError => 1,
            ErrorType::CompileError => 2,
            ErrorType::TypeError => 3,
            ErrorType::Panic => 4,
            ErrorType::TestFailure => 5,
            ErrorType::Warning => 6,
            ErrorType::Unknown => 7,
        }
    };

    let mut ordered: Vec<_> = clusters.iter().collect();
    ordered.sort_by_key(|c| priority(c));

    ordered.iter().enumerate().map(|(i, c)| {
        let action = c.suggested_fix.as_deref().unwrap_or("Investigate manually");
        format!("{}. [{}] {} — {}", i + 1, c.error_type_label(), c.root_cause, action)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::parse_errors;

    #[test]
    fn triage_empty_input() {
        let errors = parse_errors("");
        let result = triage(&errors);
        assert!(result.clusters.is_empty());
        assert!(result.fix_order.is_empty());
        assert_eq!(result.total_errors, 0);
        assert_eq!(result.total_warnings, 0);
    }

    #[test]
    fn triage_clusters_same_type() {
        let output = "error[E0432]: unresolved import `crate::foo`\nerror[E0432]: unresolved import `crate::bar`\n";
        let errors = parse_errors(output);
        let result = triage(&errors);
        assert!(!result.clusters.is_empty());
        assert!(result.total_errors > 0);
    }

    #[test]
    fn fix_order_imports_before_tests() {
        let output = "test my_test ... FAILED\nerror[E0432]: unresolved import `crate::foo`\n";
        let errors = parse_errors(output);
        let result = triage(&errors);
        // Import errors should come before test failures in fix order
        if result.fix_order.len() >= 2 {
            assert!(result.fix_order[0].contains("import"));
        }
    }

    #[test]
    fn triage_with_warnings_still_passes() {
        let output = "warning: unused variable `x`\n";
        let errors = parse_errors(output);
        let result = triage(&errors);
        assert_eq!(result.total_warnings, 1);
        assert_eq!(result.total_errors, 0);
    }

    #[test]
    fn merge_locations_pairs_error_with_location() {
        let errors = vec![
            ParsedError {
                file: None, line: None, column: None,
                message: "unresolved import".into(),
                error_type: ErrorType::ImportError,
                severity: Severity::Error,
                raw_line: "error: unresolved import".into(),
            },
            ParsedError {
                file: Some("src/main.rs".into()), line: Some(3), column: Some(5),
                message: String::new(),
                error_type: ErrorType::CompileError,
                severity: Severity::Info,
                raw_line: " --> src/main.rs:3:5".into(),
            },
        ];
        let merged = merge_locations(&errors);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].file, Some("src/main.rs".into()));
        assert_eq!(merged[0].message, "unresolved import");
    }
}

impl ErrorCluster {
    fn error_type_label(&self) -> &'static str {
        match self.error_type {
            ErrorType::ImportError => "import",
            ErrorType::TypeError => "type",
            ErrorType::TestFailure => "test",
            ErrorType::LinkError => "link",
            ErrorType::CompileError => "compile",
            ErrorType::Warning => "warning",
            ErrorType::Panic => "panic",
            ErrorType::Unknown => "unknown",
        }
    }
}
