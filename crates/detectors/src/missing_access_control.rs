use cosmwasm_guard::ast::EntryPointKind;
use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects execute handlers without info.sender authorization checks
pub struct MissingAccessControl;

/// Visitor that searches for info.sender usage in expressions
struct SenderCheckSearcher {
    found_sender_check: bool,
}

impl<'ast> Visit<'ast> for SenderCheckSearcher {
    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if let syn::Member::Named(ident) = &node.member {
            if ident == "sender" {
                // Check if base is `info`
                if is_info_expr(&node.base) {
                    self.found_sender_check = true;
                }
            }
        }
        syn::visit::visit_expr_field(self, node);
    }

    // Also check for ensure_eq!, require! macros that commonly gate access
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        let macro_name = node
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        if macro_name == "ensure_eq"
            || macro_name == "ensure"
            || macro_name == "require"
            || macro_name == "assert_eq"
        {
            // Check if tokens contain "info" and "sender"
            let tokens = node.tokens.to_string();
            if tokens.contains("info") && tokens.contains("sender") {
                self.found_sender_check = true;
            }
        }
    }
}

/// Check if an expression is `info` (a simple path)
fn is_info_expr(expr: &syn::Expr) -> bool {
    if let syn::Expr::Path(path) = expr {
        path.path.is_ident("info")
    } else {
        false
    }
}

impl Detector for MissingAccessControl {
    fn name(&self) -> &str {
        "missing-access-control"
    }

    fn description(&self) -> &str {
        "Detects execute handlers without info.sender authorization checks"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn confidence(&self) -> Confidence {
        Confidence::Medium
    }

    fn detect(&self, ctx: &AnalysisContext) -> Vec<Finding> {
        let mut findings = Vec::new();

        // Find execute entry points
        for ep in &ctx.contract.entry_points {
            if ep.kind != EntryPointKind::Execute {
                continue;
            }

            // Find the matching function body
            let func = ctx.contract.functions.iter().find(|f| f.name == ep.name);
            let Some(func) = func else { continue };
            let Some(body) = &func.body else { continue };

            // Check if the function body (including match arms) uses info.sender
            let mut searcher = SenderCheckSearcher {
                found_sender_check: false,
            };
            syn::visit::visit_block(&mut searcher, body);

            if !searcher.found_sender_check {
                findings.push(Finding {
                    detector_name: self.name().to_string(),
                    title: format!("Missing access control in execute handler `{}`", ep.name),
                    description: format!(
                        "Execute handler `{}` does not check `info.sender` for authorization. \
                         Any user can call this function, which may lead to unauthorized \
                         state changes or fund transfers.",
                        ep.name
                    ),
                    severity: Severity::High,
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
                        "Add an authorization check: \
                         `if info.sender != config.owner { return Err(...); }`"
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
        let contract = ContractVisitor::extract(PathBuf::from("test.rs"), &ast);
        let ir = IrBuilder::build_contract(&contract);
        let mut sources = HashMap::new();
        sources.insert(PathBuf::from("test.rs"), source.to_string());
        let ctx = AnalysisContext::new(&contract, &ir, &sources);
        MissingAccessControl.detect(&ctx)
    }

    #[test]
    fn test_detects_missing_access_control() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "missing-access-control");
    }

    #[test]
    fn test_no_finding_when_sender_checked() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                if info.sender != owner {
                    return Err(StdError::generic_err("unauthorized"));
                }
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_no_finding_for_query() {
        let source = r#"
            #[entry_point]
            pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
                Ok(Binary::default())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
