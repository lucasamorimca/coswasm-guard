use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;

/// Detects execute entry points that accept funds without validating info.funds.
/// Missing validation lets attackers send unexpected tokens or exploit zero-fund calls.
pub struct MissingFundsValidation;

impl Detector for MissingFundsValidation {
    fn name(&self) -> &str {
        "missing-funds-validation"
    }

    fn description(&self) -> &str {
        "Detects execute handlers that don't validate info.funds"
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
            // Only check execute entry points (they receive funds via MessageInfo)
            if ep.kind != cosmwasm_guard::ast::EntryPointKind::Execute {
                continue;
            }

            // Check if the function body references "funds"
            let has_funds_check = ctx
                .contract
                .functions
                .iter()
                .find(|f| f.name == ep.name)
                .and_then(|f| f.body.as_ref())
                .is_some_and(|body| body_references_funds(body));

            if !has_funds_check {
                findings.push(Finding {
                    detector_name: self.name().to_string(),
                    title: format!(
                        "Execute handler `{}` does not validate `info.funds`",
                        ep.name
                    ),
                    description: "Execute handlers should validate `info.funds` to prevent \
                        unexpected token deposits or ensure required payment. Without validation, \
                        users may accidentally send funds that get locked in the contract."
                        .to_string(),
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
                        "Add `if !info.funds.is_empty() { return Err(...) }` for handlers \
                         that should not accept funds, or validate the expected denom and amount."
                            .to_string(),
                    ),
                    fix: None,
                });
            }
        }

        findings
    }
}

/// Check if a syn::Block references "funds" anywhere (field access, variable, etc.)
fn body_references_funds(block: &syn::Block) -> bool {
    use syn::visit::Visit;

    struct FundsSearcher {
        found: bool,
    }

    impl<'ast> Visit<'ast> for FundsSearcher {
        fn visit_ident(&mut self, ident: &'ast syn::Ident) {
            if ident == "funds" {
                self.found = true;
            }
        }

        fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
            if let syn::Member::Named(ident) = &node.member {
                if ident == "funds" {
                    self.found = true;
                    return;
                }
            }
            syn::visit::visit_expr_field(self, node);
        }
    }

    let mut searcher = FundsSearcher { found: false };
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
        MissingFundsValidation.detect(&ctx)
    }

    #[test]
    fn test_detects_missing_funds_check() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> Result<Response, ContractError> {
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "missing-funds-validation");
    }

    #[test]
    fn test_no_finding_when_funds_checked() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> Result<Response, ContractError> {
                if !info.funds.is_empty() {
                    return Err(ContractError::NoFundsExpected {});
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
            pub fn query(deps: Deps, env: Env, msg: QueryMsg)
                -> StdResult<Binary> {
                Ok(Binary::default())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
