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
| 9 | ✅ Complete | Advanced detectors (7 new detectors) | v0.2.0 |
| 10 | ✅ Complete | Developer experience (config, suppression, --audit) | v0.3.0 |
| 11 | ✅ Complete | Performance & caching | v0.3.0 |

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

## Phase 9: Advanced Detectors (Complete)

**New detectors added (7 total, 10 detectors overall):**
- storage-key-collision: Duplicate storage keys across state items (High/High)
- unsafe-unwrap: Unwrap/expect in non-test code (Medium/High)
- arithmetic-overflow: Wrapping ops (neg/wrapping_add) CWA-2024-002 (High/Medium)
- missing-error-propagation: Discarded Result from function calls (Low/High)
- submessage-reply-unvalidated: Reply handler without msg.id check (High/Medium)
- nondeterministic-iteration: HashMap iteration without sorting (Medium/Medium)
- incorrect-permission-hierarchy: Admin storage write without ownership check (Medium/Medium)
- Test suite: 34 detector unit tests + 5 integration tests (59 total)

## Phase 10: Developer Experience (Complete)

**DX features added:**
- Config system: `.cosmwasm-guard.toml` per-detector enable/disable, file exclusions
- Inline suppression: `// cosmwasm-guard-ignore: detector-name` comment syntax
- `--config` flag for custom config paths
- `--audit` flag for maximum coverage + lower confidence threshold
- `--init` flag to generate default config template
- SARIF fixes array populated with FixSuggestion data
- Fix suggestions: `.unwrap()` → `?` transformation, `let _ = call()` → `.ok()`
- GitHub Action: composite action at `.github/actions/cosmwasm-guard/action.yml`
- Test suite: 23 core unit tests + 39 detector tests + 5 integration (62 total)

## Phase 11: Performance & Caching (Complete)

**Performance optimizations implemented:**
- File-level cache: SHA256 hash → bincode CachedFileArtifact per .rs file
- Cache directory: `{crate_path}/.cosmwasm-guard-cache/` with manifest.json + artifacts/
- Schema version in manifest for auto-invalidation
- `--no-cache` CLI flag to disable caching when needed
- New `analyze_crate_cached()` API; `analyze_crate()` preserved for backwards compatibility
- `CrateAnalysis` struct bundles contract + ir + source_map
- Parallel detection infrastructure: Rayon scopes with Mutex (disabled at runtime due to proc-macro2 span-locations incompatibility)
- Threshold guard: >3 detectors for parallelism (set to MAX due to technical constraints)
- Dependencies: rayon 1.10, bincode 1, sha2 0.10
- ContractIr/FunctionIr derive Clone + Serialize + Deserialize
- Test suite: 25 core unit tests + 34 detector tests + 5 integration (64 total)
- Manual testing: Cache works correctly, measured improvement in incremental analysis

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
