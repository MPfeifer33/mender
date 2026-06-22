use crate::triage::TriageResult;
use crate::MenderError;

pub fn print_triage(result: &TriageResult, is_json: bool) -> Result<(), MenderError> {
    if is_json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "triage": {
                "total_errors": result.total_errors,
                "total_warnings": result.total_warnings,
                "clusters": result.clusters,
                "fix_order": result.fix_order,
            }
        }))?);
    } else {
        if result.clusters.is_empty() {
            println!("mender: no errors found in output.");
            return Ok(());
        }

        println!("mender triage: {} error(s), {} warning(s) in {} cluster(s)",
            result.total_errors, result.total_warnings, result.clusters.len());
        println!();

        for cluster in &result.clusters {
            let icon = match cluster.severity {
                crate::parse::Severity::Error => "✗",
                crate::parse::Severity::Warning => "⚠",
                crate::parse::Severity::Info => "·",
            };

            let file_str = cluster.file.as_deref().unwrap_or("(no file)");
            println!("  {icon} {} — {} ({}x)", cluster.root_cause, file_str, cluster.count);

            if let Some(ref fix) = cluster.suggested_fix {
                println!("    Fix: {fix}");
            }
        }

        if !result.fix_order.is_empty() {
            println!();
            println!("  Suggested fix order:");
            for step in &result.fix_order {
                println!("    {step}");
            }
        }
    }
    Ok(())
}

pub fn print_patterns(is_json: bool) -> Result<(), MenderError> {
    let patterns = vec![
        ("Rust compile errors", "error[E0xxx]: ...", "compile_error"),
        ("Rust test failures", "test xxx ... FAILED", "test_failure"),
        ("Rust panics", "thread 'xxx' panicked at ...", "panic"),
        ("Rust warnings", "warning: ...", "warning"),
        ("TypeScript errors", "file.ts(line,col): error TSxxxx: ...", "type_error"),
        ("Python errors", "XxxError: message", "compile_error"),
        ("Python import errors", "ImportError: ...", "import_error"),
        ("Go errors", "file.go:line:col: message", "compile_error"),
        ("Generic errors", "error: / fatal:", "unknown"),
    ];

    if is_json {
        let entries: Vec<serde_json::Value> = patterns.iter()
            .map(|(name, example, category)| serde_json::json!({
                "name": name,
                "example": example,
                "category": category,
            }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "patterns": entries,
        }))?);
    } else {
        println!("mender: supported error patterns");
        println!();
        for (name, example, _) in &patterns {
            println!("  {name}");
            println!("    Example: {example}");
            println!();
        }
    }
    Ok(())
}
