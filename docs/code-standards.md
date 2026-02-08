# Code Standards & Guidelines

## File Organization

### Naming Conventions
- **Files:** snake_case (e.g., `missing_addr_validate.rs`, `contract_info.rs`)
- **Modules:** snake_case matching file names
- **Types:** PascalCase (Detector, ContractInfo, SsaVar)
- **Functions/variables:** snake_case
- **Constants:** SCREAMING_SNAKE_CASE

### File Size Limits
- **Code files:** Max 200 lines per file (refactor if exceeded)
  - Split large modules into focused sub-modules
  - Extract common utilities into dedicated files
  - Use composition for complex components

- **Documentation files:** Max 100-120 lines (split into directories)

### Module Structure Example
```
crates/core/src/
├── ast/
│   ├── mod.rs (public exports)
│   ├── parser.rs
│   ├── contract_info.rs
│   ├── visitor.rs
│   └── utils.rs
├── ir/
│   ├── mod.rs
│   ├── cfg.rs
│   ├── instruction.rs
│   └── builder.rs
└── lib.rs (workspace root exports)
```

## Code Quality Standards

### Required for All Code
- **No syntax errors** — Code must compile without warnings
- **Clippy clean** — Run `cargo clippy` before commit
- **License headers:** Include Apache-2.0 at file top (optional)
- **Error handling:** Use `thiserror`, `anyhow` for structured errors
- **Panic-free:** Avoid panics; use Result<T, E> instead

### Documentation
- Public APIs: Document with `///` doc comments
- Complex logic: Add inline comments explaining "why"
- Examples: Include code examples in doc comments for traits

### Testing
- Unit tests for parsers, IR builders, detectors
- Integration tests for end-to-end analysis
- Test coverage: Aim for 70%+ on core modules

## Dependencies (Workspace)

| Crate | Purpose |
|-------|---------|
| syn 2 | AST parsing with full features |
| quote, proc-macro2 | Code generation support |
| clap 4 | CLI argument parsing |
| serde/serde_json | Serialization (JSON output) |
| colored | Terminal colors |
| walkdir | Recursive file discovery |
| thiserror | Structured error types |
| anyhow | Error context |

## Workspace Configuration

**Edition:** 2021
**License:** Apache-2.0
**Authors:** SafeStack AI
**Version:** 0.1.0 (workspace-wide)

## Pre-commit Checklist

- [ ] Code compiles: `cargo check`
- [ ] No clippy warnings: `cargo clippy`
- [ ] Tests pass: `cargo test`
- [ ] File size under 200 lines (code) / 120 lines (docs)
- [ ] No hardcoded credentials or secrets
