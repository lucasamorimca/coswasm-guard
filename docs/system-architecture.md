# System Architecture

## Overview

cosmwasm-guard is a Cargo workspace with 3 crates implementing an analysis pipeline:

```
Source Code
    ↓
Parser (syn) → ContractInfo
    ↓
IR Builder → SSA IR (CFG + instructions)
    ↓
Detector Registry
    ↓
Findings Aggregator
    ↓
Output Formatters (text/JSON/SARIF)
```

## Workspace Structure

### crates/core
Core analysis engine with:
- **ast** — Parsing (syn), ContractInfo model, crate analyzer
- **ir** — SSA IR builder, CFG construction, instruction types
- **detector** — Detector trait, analysis context, registry
- **finding** — Severity/confidence enums, Finding struct, display formats
- **report** — Report aggregation and serialization

### crates/detectors
Built-in vulnerability detectors:
- `missing_addr_validate` — Pattern matching for unvalidated addresses
- `missing_access_control` — Handler authorization analysis
- `unbounded_iteration` — Map range iteration detection

### crates/cli
CLI binary (cosmwasm-guard):
- **commands** — analyze, list subcommands
- **output** — text (colored), JSON, SARIF 2.1.0 formatting
- **main.rs** — Clap argument parsing, orchestration

## Key Components

### ContractInfo (ast/contract_info.rs)
Unified model extracted from all .rs files in a crate:
- Function definitions with entry point markers (includes param-type-based kind inference)
- Type definitions (structs, enums)
- Module structure
- Efficient extraction via owned syn::File (eliminates AST cloning)

### SSA IR (ir/)
- **Instruction** — Operations with operands (binary/unary ops, calls, literals)
- **Cfg** — Basic blocks + edges for control flow
- **FunctionIr** — Per-function IR with data dependencies
- **ContractIr** — All functions + metadata
- **Path Resolver** — Avoids phantom SSA vars for enum variants/type paths (Phase 8 hardening)

### AnalysisContext (detector/context.rs)
Passed to detectors:
- `contract: ContractInfo` — Parsed contract model
- `ir: ContractIr` — Full SSA IR
- `raw_asts: Vec<syn::File>` — Original AST access

### Finding (finding/types.rs)
Structured vulnerability report:
- detector_name, title, description
- severity: [High, Medium, Low, Informational]
- confidence: [High, Medium, Low]
- location: file, line range, column range
- snippet: source code context

## Data Flow

1. **Parse:** walkdir finds .rs files → syn::parse_file() per file
2. **Extract:** Visitor collects functions, types → ContractInfo
3. **Build IR:** CFG construction, def-use analysis
4. **Detect:** Each detector queries IR/AST patterns
5. **Report:** Findings aggregated, formatted, output

## Known Limitations & Phase 8 Resolutions

| Issue | Status | Notes |
|-------|--------|-------|
| Phantom SSA vars for enum variants | ✅ Resolved (Phase 8 H1) | Path resolver now avoids creating unnecessary vars |
| AST cloning overhead in extract() | ✅ Resolved (Phase 8 H5) | Changed to owned syn::File parameter |
| Non-standard entry point detection | ✅ Resolved (Phase 8 M2) | Now infers kind from param types if name doesn't match |
| Unbounded iteration false positives | ✅ Resolved (Phase 8 M4) | Only flags .range() on storage Map/IndexedMap |
| Access control dispatch limitation | ✅ Partially resolved (Phase 8 H6) | Now follows match arm dispatch; non-match dispatch requires advanced analysis |
