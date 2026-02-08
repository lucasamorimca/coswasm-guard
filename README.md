# cosmwasm-guard

AST-based static analysis framework for CosmWasm smart contracts.

## Features

- **Multi-file crate analysis** — Parses all `.rs` files and merges into a unified contract model
- **SSA-form IR** — Full intermediate representation with CFG, def-use chains, and CosmWasm-specific opcodes
- **Pluggable detectors** — Simple `Detector` trait for writing custom vulnerability checks
- **3 built-in detectors** — Missing address validation, missing access control, unbounded iteration
- **Multiple output formats** — Colored terminal, JSON, SARIF 2.1.0 (GitHub Code Scanning ready)
- **CI-friendly** — Non-zero exit code when findings exceed severity threshold

## Installation

```bash
cargo install --path crates/cli
```

## Usage

```bash
# Analyze a CosmWasm contract crate
cosmwasm-guard analyze ./path/to/contract

# JSON output
cosmwasm-guard analyze ./path/to/contract --format json

# SARIF output for GitHub Code Scanning
cosmwasm-guard analyze ./path/to/contract --format sarif > results.sarif

# Filter by severity
cosmwasm-guard analyze ./path/to/contract --severity high

# Run specific detectors only
cosmwasm-guard analyze ./path/to/contract --detectors missing-addr-validate,missing-access-control

# List available detectors
cosmwasm-guard list
```

## Built-in Detectors

| Detector | Severity | Description |
|----------|----------|-------------|
| `missing-addr-validate` | Medium | String fields with address-like names not validated via `addr_validate()` |
| `missing-access-control` | High | Execute handlers without `info.sender` authorization checks |
| `unbounded-iteration` | Medium | `Map::range()` calls without `.take()` limit, risking gas exhaustion |

## Architecture

Cargo workspace with 3 crates:

```
crates/
  core/       — AST parsing, IR, contract model, detector trait, reporting
  detectors/  — Built-in vulnerability detectors
  cli/        — Command-line interface (cosmwasm-guard binary)
```

**Pipeline:** `Source Files → syn::parse_file() → ContractInfo → SSA IR → Detectors → Findings → Output`

## Writing Custom Detectors

Implement the `Detector` trait:

```rust
use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;

pub struct MyDetector;

impl Detector for MyDetector {
    fn name(&self) -> &str { "my-detector" }
    fn description(&self) -> &str { "Detects something dangerous" }
    fn severity(&self) -> Severity { Severity::High }
    fn confidence(&self) -> Confidence { Confidence::Medium }

    fn detect(&self, ctx: &AnalysisContext) -> Vec<Finding> {
        // Access ctx.contract for parsed contract info
        // Access ctx.ir for SSA IR with CFG
        // Access ctx.raw_asts() for syn AST pattern matching
        vec![]
    }
}
```

## License

Apache-2.0
