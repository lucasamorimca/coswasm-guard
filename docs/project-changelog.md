# Project Changelog

## v0.4.0 - Phase 12 Detector Validation & False Positive Reduction

**Release Date:** February 2026

### Real-World Validation
- **Baseline Test**: Copied 6 representative cw-plus contract files into `crates/cli/tests/fixtures/real-world/`
- **Initial Run**: All 13 detectors executed without crashes, identified 5 baseline findings

### False Positive Reduction (80% improvement)
- **missing-error-propagation**: Added #[cfg(test)] module skipping to exclude test utilities from analysis
- **unsafe-unwrap**: Added safe method chain detection (.unwrap_or, .unwrap_or_default, .unwrap_or_else) to avoid flagging non-panicking unwraps
- **missing-access-control**: Expanded allowlist to recognize cw_ownable patterns (assert_owner, is_owner, Ownable trait methods) and ensure_eq!/ensure! macros with owner/admin checks
- **missing-funds-validation**: Lowered confidence from Medium to Low (most execute handlers legitimately skip funds checks) and added recognition for cw_utils helpers (must_pay, nonpayable, one_coin)
- **uninitialized-state-access**: Verified that may_load() calls are properly excluded from analysis

### Metrics
- Real-world validation results: 5 → 1 findings (1 true positive + 0 false positives)
- Test count: 73 → 84 (11 new regression tests)
- Code quality: 0 compiler warnings
- All existing tests passing with no regressions

### Testing
- 84 tests total (25 core + 54 detector + 5 integration)
- New integration test: `real_world_validation.rs` with cw-plus contracts
- 11 new detector unit tests for FP fixes

---

## v0.3.0 - Phase 11 Performance & Caching

**Release Date:** February 2026

### Features
- **File-level caching:** SHA256 hash-based cache for ContractInfo + IR artifacts per .rs file
- **Cache persistence:** `.cosmwasm-guard-cache/` directory with manifest.json + bincode artifacts
- **Cache invalidation:** Automatic schema versioning in manifest for safe upgrades
- **Incremental analysis:** Only re-analyze changed files since last run
- **New API:** `analyze_crate_cached()` for cache-aware analysis; `analyze_crate()` unchanged for compatibility

### Performance
- Cached re-run of unchanged crate: <200ms (tested on 10-file fixture)
- First-run analysis: <3s for typical contract crate
- Parallel detector infrastructure: Rayon scopes with Mutex (disabled at runtime due to proc-macro2 span-locations incompatibility with threading)

### CLI Changes
- `--no-cache` flag to bypass cache when needed
- Cache directory: `{crate_path}/.cosmwasm-guard-cache/`

### Dependencies
- rayon 1.10 (parallel infrastructure)
- bincode 1 (artifact serialization)
- sha2 0.10 (file hashing)

### Serialization
- ContractIr, FunctionIr now derive Clone + Serialize + Deserialize
- Cache manifest schema versioning prevents corrupted data across upgrades

### Testing
- 64 tests total (25 core + 34 detector + 5 integration)
- Manual verification: cache invalidation on file change, incremental analysis correctness

### Known Limitations
- Rayon parallelism disabled at runtime — proc-macro2 span-locations feature incompatible with Rayon threads
- Threshold guard set to MAX (parallelism disabled) pending proc-macro2 workaround

---

## v0.3.0 - Phase 10 Developer Experience

**Release Date:** February 2026

### Features
- **Config system:** `.cosmwasm-guard.toml` with per-detector enable/disable and file exclusion globs
- **Inline suppression:** `// cosmwasm-guard-ignore: detector-name` comment syntax + wildcard suppression
- **Audit mode:** `--audit` flag for maximum detector coverage with lower confidence threshold
- **Config initialization:** `--init` flag to generate default config template

### CLI Changes
- `--config <path>` flag to specify custom config file
- `--audit` mode enables all detectors and suppresses confidence-based filtering
- `--init` generates `.cosmwasm-guard.toml` template in current directory

### SARIF Output
- Fixes array in SARIF report populated when Finding has fix suggestions
- Fix suggestions include replacement_text + location for IDE integration

### Fix Suggestions
- unsafe-unwrap: `.unwrap()` → `?` transformation
- missing-error-propagation: `let _ = call()` → `.ok()`

### GitHub Integration
- GitHub Action: composite action at `.github/actions/cosmwasm-guard/action.yml`
- Full SARIF 2.1.0 support for GitHub Code Scanning integration

### Testing
- 62 tests total (23 core + 39 detector + 5 integration)
- New integration tests: config disable, suppression handling, audit mode

---

## v0.2.0 - Phase 9 Advanced Detectors

**Release Date:** February 2026

### New Detectors (7 total, 10 detectors overall)
- **storage-key-collision:** Duplicate storage keys across state items (High severity, High confidence)
- **unsafe-unwrap:** `.unwrap()` and `.expect()` in non-test code (Medium severity, High confidence)
- **arithmetic-overflow:** Wrapping operations (`.neg()`, `.wrapping_add()`) — CWA-2024-002 (High severity, Medium confidence)
- **missing-error-propagation:** Discarded Result via `let _ = call()` (Low severity, High confidence)
- **submessage-reply-unvalidated:** Reply handler without msg.id validation (High severity, Medium confidence)
- **nondeterministic-iteration:** HashMap iteration without sorting (Medium severity, Medium confidence)
- **incorrect-permission-hierarchy:** Admin storage write without ownership check (Medium severity, Medium confidence)

### Architecture
- Detectors now operate on raw AST via `ctx.raw_asts()` for pattern matching
- Method chain analysis: `collect_method_chain()` walks receiver chain for `.range()`, `.take()`
- Address field heuristic: name contains addr/owner/recipient/admin/etc + type is String
- Storage-qualified `.range()` check: verifies receiver against known Map/IndexedMap names

### Testing
- 59 tests total (25 core + 34 detector unit tests + 5 integration)
- 3 regression tests for M4 (storage qualification) + H6 (dispatch following)

---

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
