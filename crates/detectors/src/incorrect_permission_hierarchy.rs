use cosmwasm_guard::ast::EntryPointKind;
use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects functions that write to admin/owner/config storage without
/// verifying the caller against the stored admin. Extends missing-access-control
/// with more nuanced permission checks.
pub struct IncorrectPermissionHierarchy;

/// Names that indicate admin/config storage items
const ADMIN_STORAGE_PATTERNS: &[&str] = &["config", "admin", "owner", "governance"];

/// Visitor that checks for storage writes to admin items and sender verification
struct PermissionSearcher {
    writes_admin_storage: bool,
    checks_stored_admin: bool,
    admin_item_names: Vec<String>,
}

impl<'ast> Visit<'ast> for PermissionSearcher {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        // Check for writes to admin-like storage items
        if method == "save" || method == "update" {
            if let syn::Expr::Path(path) = node.receiver.as_ref() {
                if let Some(name) = path.path.segments.last() {
                    let name_lower = name.ident.to_string().to_lowercase();
                    if ADMIN_STORAGE_PATTERNS.iter().any(|p| name_lower.contains(p)) {
                        self.writes_admin_storage = true;
                        self.admin_item_names
                            .push(name.ident.to_string());
                    }
                }
            }
        }

        // Check for loading admin/config to compare against sender
        if method == "load" || method == "may_load" {
            if let syn::Expr::Path(path) = node.receiver.as_ref() {
                if let Some(name) = path.path.segments.last() {
                    let name_lower = name.ident.to_string().to_lowercase();
                    if ADMIN_STORAGE_PATTERNS.iter().any(|p| name_lower.contains(p)) {
                        self.checks_stored_admin = true;
                    }
                }
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

impl Detector for IncorrectPermissionHierarchy {
    fn name(&self) -> &str {
        "incorrect-permission-hierarchy"
    }

    fn description(&self) -> &str {
        "Detects admin storage writes without verifying caller against stored admin"
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn confidence(&self) -> Confidence {
        Confidence::Medium
    }

    fn detect(&self, ctx: &AnalysisContext) -> Vec<Finding> {
        let mut findings = Vec::new();

        for ep in &ctx.contract.entry_points {
            if ep.kind != EntryPointKind::Execute {
                continue;
            }

            let func = ctx.contract.functions.iter().find(|f| f.name == ep.name);
            let Some(func) = func else { continue };
            let Some(body) = &func.body else { continue };

            let mut searcher = PermissionSearcher {
                writes_admin_storage: false,
                checks_stored_admin: false,
                admin_item_names: Vec::new(),
            };
            syn::visit::visit_block(&mut searcher, body);

            if searcher.writes_admin_storage && !searcher.checks_stored_admin {
                findings.push(Finding {
                    detector_name: self.name().to_string(),
                    title: format!(
                        "Admin storage write without ownership verification in `{}`",
                        ep.name
                    ),
                    description: format!(
                        "Execute handler `{}` writes to admin storage ({}) without \
                         loading and verifying the current admin/owner. Any caller \
                         could overwrite the admin configuration.",
                        ep.name,
                        searcher.admin_item_names.join(", ")
                    ),
                    severity: Severity::Medium,
                    confidence: Confidence::Medium,
                    locations: vec![SourceLocation {
                        file: ep.span.file.clone(),
                        start_line: ep.span.start_line,
                        end_line: ep.span.end_line,
                        start_col: ep.span.start_col,
                        end_col: ep.span.end_col,
                        snippet: None,
                    }],
                    recommendation: Some(
                        "Load the current admin/config and verify `info.sender` \
                         matches before updating."
                            .to_string(),
                    ),
                });
            }
        }

        findings
    }
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
        IncorrectPermissionHierarchy.detect(&ctx)
    }

    #[test]
    fn test_detects_unverified_admin_write() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                CONFIG.save(deps.storage, &new_config)?;
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "incorrect-permission-hierarchy");
    }

    #[test]
    fn test_no_finding_with_admin_check() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                let config = CONFIG.load(deps.storage)?;
                if info.sender != config.owner {
                    return Err(StdError::generic_err("unauthorized"));
                }
                CONFIG.save(deps.storage, &new_config)?;
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_no_finding_for_non_admin_write() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                BALANCES.save(deps.storage, &sender, &amount)?;
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
