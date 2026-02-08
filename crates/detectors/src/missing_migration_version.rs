use cosmwasm_guard::ast::EntryPointKind;
use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects migrate() entry points without cw2 version tracking.
/// Without set_contract_version or ensure_from_older, contracts can be
/// downgraded or lose version history, breaking upgrade safety.
pub struct MissingMigrationVersion;

impl Detector for MissingMigrationVersion {
    fn name(&self) -> &str {
        "missing-migration-version"
    }

    fn description(&self) -> &str {
        "Detects migrate handlers without cw2 version tracking"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn confidence(&self) -> Confidence {
        Confidence::High
    }

    fn detect(&self, ctx: &AnalysisContext) -> Vec<Finding> {
        let mut findings = Vec::new();

        for ep in &ctx.contract.entry_points {
            if ep.kind != EntryPointKind::Migrate {
                continue;
            }

            let has_version_call = ctx
                .contract
                .functions
                .iter()
                .find(|f| f.name == ep.name)
                .and_then(|f| f.body.as_ref())
                .is_some_and(|body| body_has_version_call(body));

            if !has_version_call {
                findings.push(Finding {
                    detector_name: self.name().to_string(),
                    title: format!(
                        "Migrate handler `{}` missing version tracking",
                        ep.name
                    ),
                    description: "The migrate handler does not call `set_contract_version` or \
                        `ensure_from_older_version`. Without version tracking, the contract \
                        can be downgraded to an older version, potentially reintroducing \
                        patched vulnerabilities."
                        .to_string(),
                    severity: Severity::High,
                    confidence: Confidence::High,
                    locations: vec![SourceLocation {
                        file: ep.span.file.clone(),
                        start_line: ep.span.start_line,
                        end_line: ep.span.end_line,
                        start_col: ep.span.start_col,
                        end_col: ep.span.end_col,
                        snippet: None,
                    }],
                    recommendation: Some(
                        "Add `cw2::set_contract_version(deps.storage, CONTRACT_NAME, \
                         CONTRACT_VERSION)?;` at the start of the migrate handler, or use \
                         `cw2::ensure_from_older_version(...)` to enforce upgrade-only migrations."
                            .to_string(),
                    ),
                    fix: None,
                });
            }
        }

        findings
    }
}

/// Check if a block contains calls to cw2 version functions
fn body_has_version_call(block: &syn::Block) -> bool {
    struct VersionCallSearcher {
        found: bool,
    }

    impl<'ast> Visit<'ast> for VersionCallSearcher {
        fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
            if let syn::Expr::Path(path) = node.func.as_ref() {
                let path_str = path
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::");
                if path_str.contains("set_contract_version")
                    || path_str.contains("ensure_from_older")
                {
                    self.found = true;
                }
            }
            syn::visit::visit_expr_call(self, node);
        }

        fn visit_ident(&mut self, ident: &'ast syn::Ident) {
            let name = ident.to_string();
            if name == "set_contract_version" || name.starts_with("ensure_from_older") {
                self.found = true;
            }
        }
    }

    let mut searcher = VersionCallSearcher { found: false };
    syn::visit::visit_block(&mut searcher, block);
    searcher.found
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_guard::ast::{parse_source, ContractVisitor};
    use cosmwasm_guard::ir::builder::IrBuilder;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn analyze(source: &str) -> Vec<Finding> {
        let ast = parse_source(source).unwrap();
        let contract = ContractVisitor::extract(PathBuf::from("test.rs"), ast);
        let ir = IrBuilder::build_contract(&contract);
        let mut sources = HashMap::new();
        sources.insert(PathBuf::from("test.rs"), source.to_string());
        let ctx = AnalysisContext::new(&contract, &ir, &sources);
        MissingMigrationVersion.detect(&ctx)
    }

    #[test]
    fn test_detects_missing_version() {
        let source = r#"
            #[entry_point]
            pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg)
                -> Result<Response, ContractError> {
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "missing-migration-version");
    }

    #[test]
    fn test_no_finding_with_set_version() {
        let source = r#"
            #[entry_point]
            pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg)
                -> Result<Response, ContractError> {
                set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_no_finding_with_cw2_qualified() {
        let source = r#"
            #[entry_point]
            pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg)
                -> Result<Response, ContractError> {
                cw2::set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
