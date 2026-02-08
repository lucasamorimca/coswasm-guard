# Project Changelog

## v0.1.0 - Phase 8 Hardening & Known Issues Resolution

**Release Date:** February 2026

### Improvements
- **H1 - Path Resolver:** Eliminated phantom SSA variable generation for enum variants and type paths, improving IR accuracy
- **H5 - AST Handling:** Modified extract() to take owned syn::File instead of borrowing, eliminating unnecessary AST clones
- **M2 - Entry Point Detection:** Enhanced entry point kind inference to use parameter types when function names are non-standard
- **M4 - Unbounded Iteration:** Refined detector to only flag .range() calls on CosmWasm storage containers (Map/IndexedMap), reducing false positives
- **H6 - Access Control:** Implemented dispatch-following in access control detector to trace handler function calls through match arms

### Testing
- Added 8 new regression tests
- Total test suite: 35 tests (up from 27)
  - 15 core unit tests
  - 9 detector unit tests
  - 11 integration tests (includes new regression tests)

### Known Limitations (Documented)
- IR builder creates phantom SSA vars for unhandled type paths (low impact edge case)
- ContractVisitor extracts full AST with temporary cloning overhead (mitigated in Phase 8)
- Access control detector does not follow non-match dispatch patterns (requires advanced control flow analysis)
- Entry point kind inference only matches exact names or uses param types (heuristic-based)
- Unbounded iteration detector flags all .range() calls on any type (refined in Phase 8 to storage types only)

---

## v0.1.0 - Phase 1-7 (MVP)

**Phases 1-7 delivered core framework and MVP detectors.**

### Core Infrastructure
- Workspace setup with 3 crates: core, detectors, cli
- syn-based AST parsing with full Rust syntax support
- ContractInfo extraction via visitor pattern
- Multi-file crate analysis

### Intermediate Representation
- SSA form IR with basic blocks and control flow graphs
- Def-use chains and dataflow metadata
- CosmWasm-specific opcode modeling

### Detectors (MVP - 3 built-in)
- missing-addr-validate (Medium severity)
- missing-access-control (High severity)
- unbounded-iteration (Medium severity)

### Output Formats
- Colored terminal output
- JSON serialization
- SARIF 2.1.0 export for GitHub Code Scanning

### CLI Binary
- cosmwasm-guard binary with analyze/list subcommands
- Severity filtering and exit code support
- Integration tests for end-to-end analysis
