# Development Roadmap

## Phase Overview

| Phase | Status | Focus | Completion |
|-------|--------|-------|-----------|
| 1 | ✅ Complete | Project setup, workspace structure | v0.1.0 |
| 2 | ✅ Complete | AST parsing, ContractInfo extraction | v0.1.0 |
| 3 | ✅ Complete | SSA IR, CFG, instruction types | v0.1.0 |
| 4 | ✅ Complete | Detector trait, analysis context | v0.1.0 |
| 5 | ✅ Complete | 3 built-in detectors (MVP) | v0.1.0 |
| 6 | ✅ Complete | Multi-format output (text/JSON/SARIF) | v0.1.0 |
| 7 | ✅ Complete | CLI binary, integration tests | v0.1.0 |
| 8 | ✅ Complete | Hardening & known issues resolution | v0.1.0 |

## Completed Deliverables (Phase 1-7)

### Core Infrastructure
- ✅ Cargo workspace with core/detectors/cli crates
- ✅ Multi-file crate analysis via analyze_crate()
- ✅ syn-based AST parsing with full Rust syntax support
- ✅ Module and function extraction visitor
- ✅ 35 passing tests (15 core + 9 detectors + 11 integration)

### IR & Analysis
- ✅ SSA intermediate representation (SsaVar, Instruction)
- ✅ Control flow graph (BasicBlock, Cfg, BlockId)
- ✅ Def-use chains and dataflow metadata
- ✅ CosmWasm-specific opcode modeling

### Detectors (MVP)
- ✅ missing-addr-validate (Medium severity)
- ✅ missing-access-control (High severity)
- ✅ unbounded-iteration (Medium severity)

### Output
- ✅ Colored terminal output (text formatter)
- ✅ JSON serialization (machine-parseable)
- ✅ SARIF 2.1.0 export (GitHub Code Scanning)
- ✅ Severity filtering and exit codes

### CLI
- ✅ cosmwasm-guard binary
- ✅ `analyze` subcommand with filtering options
- ✅ `list` subcommand for detector inventory

## Phase 8: Hardening & Known Issues (Complete)

**Improvements made:**
- H1: Path resolver avoids phantom SSA vars for enum variants/type paths
- H5: extract() takes owned syn::File instead of borrowing (eliminates AST clone)
- M2: Entry point kind now inferred from param types when fn name is non-standard
- M4: Unbounded iteration detector only flags .range() on storage Map/IndexedMap
- H6: Access control detector follows match arm dispatch to handler functions
- 8 new regression tests added (35 total tests)

## Future Work (Post-MVP)

### Phase 9: Taint Analysis
**Goal:** Track untrusted input flows through contract

- Implement source/sink taint propagation
- Model CosmWasm message boundary as source
- Detect unsanitized user data in critical operations

### Phase 10: Advanced Detectors (5+ more)
Potential detectors:
- Reentrancy patterns (callback loops)
- Unsafe numeric operations (overflow/underflow)
- State consistency violations
- Incorrect permission hierarchies
- Improper error handling

### Phase 11: Plugin System
- Detector registration via external crates
- Custom detector distribution via crates.io
- Detector version management

### Phase 12: Constraint Solver (Optional)
- Path-sensitive analysis for conditions
- Numeric constraint propagation
- Reduce false positives on conditional checks

## Success Metrics

- Phase 7: 90%+ true positive rate on 20+ test contracts
- Phase 8+: Zero false negatives on known vulnerability classes
- Framework extensibility: 5+ third-party detectors adopted
