# Project Overview & PDR

## Purpose

**cosmwasm-guard** is an AST-based static analysis framework for Rust-based CosmWasm smart contracts. It functions as Slither equivalent for the Cosmos ecosystem, detecting common vulnerabilities and anti-patterns before contracts reach mainnet.

## Project Scope

**In-Scope:**
- Multi-file Rust crate analysis with unified contract model extraction
- SSA form intermediate representation (IR) with control flow graphs (CFG)
- Plugin-based vulnerability detector framework
- 3 built-in detectors (missing-addr-validate, missing-access-control, unbounded-iteration)
- Multiple output formats (terminal, JSON, SARIF 2.1.0)
- CI/CD integration via non-zero exit on severity threshold breach

**Out-of-Scope (Future Work):**
- Taint flow analysis
- Formal verification / symbolic execution
- Real-time monitoring or runtime analysis
- Bytecode-level analysis

## Key Technical Decisions

| Decision | Rationale |
|----------|-----------|
| SSA IR representation | Enables dataflow analysis and def-use chains |
| syn crate for parsing | Full Rust AST with extras support (macros, attributes) |
| Detector trait design | Allows custom implementations without core modifications |
| Multi-format output | SARIF for tooling ecosystem; JSON for parsing; terminal for humans |
| Apache-2.0 license | Industry-standard permissive open-source |

## Success Metrics

- Framework handles 5+ detector implementations
- Analysis completes for typical contracts (<5s)
- Zero false negatives on known vulnerability patterns
- 90%+ true positive rate on test suite

## MVP Detectors

1. **missing-addr-validate** (Medium) — String address fields without validation
2. **missing-access-control** (High) — Execute handlers lacking authorization checks
3. **unbounded-iteration** (Medium) — Map::range() without take() limit

## Timeline

- Phase 1-7: Complete (MVP delivered, v0.1.0)
- Phase 8: Complete (Hardening & known issues resolution, v0.1.0)
- Phase 9+: Taint analysis, constraint solver, advanced detectors, plugin system
