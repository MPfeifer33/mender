# PROJECT.md — mender

**What:** Failure triage tool — parses build/test errors, clusters them by root cause, suggests fix order.

**Status:** MVP complete, published to github.com/MPfeifer33/mender

## Architecture
- `src/cli.rs` — Clap 4 CLI: `triage` (--file/--run/stdin), `patterns`
- `src/parse.rs` — Multi-language error extraction (Rust compile/test/panic/warning, TypeScript, Python, Go, generic). 5 unit tests.
- `src/triage.rs` — ErrorCluster with root cause detection, merge_locations(), suggest_fix(), fix order priority: imports > link > compile > type > panic > test > warning
- `src/report.rs` — Triage results and pattern listing (text + JSON)
- `src/main.rs` — Standard error handling with typed HarborError enum

## Usage
```bash
# Triage errors from a file
mender triage --file build.log

# Pipe compiler output directly
cargo build 2>&1 | mender triage

# Run a command and triage its output
mender triage --run "cargo test"

# List known error patterns
mender patterns
```

## Design Decisions
- Regex-based parsing (not tree-sitter) for MVP speed
- Fix-order prioritization so agents address root causes first
- Clusters errors by type to reduce noise

## Last Updated
June 22, 2026 — Initial MVP
