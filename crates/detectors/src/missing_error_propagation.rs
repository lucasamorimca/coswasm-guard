use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects `let _ = expr` patterns where errors are silently discarded.
/// Common source of bugs when Result values are ignored.
pub struct MissingErrorPropagation;

struct WildcardLetSearcher {
    findings: Vec<(usize, usize)>,
}

impl<'ast> Visit<'ast> for WildcardLetSearcher {
    fn visit_local(&mut self, node: &'ast syn::Local) {
        // Check for `let _ = <call_expr>` pattern
        if let syn::Pat::Wild(wild) = &node.pat {
            if let Some(init) = &node.init {
                // Only flag if RHS is a function/method call (likely fallible)
                if is_call_expr(&init.expr) {
                    let span = wild.underscore_token.span;
                    self.findings.push((span.start().line, span.start().column));
                }
            }
        }
        syn::visit::visit_local(self, node);
    }
}

fn is_call_expr(expr: &syn::Expr) -> bool {
    matches!(
        expr,
        syn::Expr::Call(_) | syn::Expr::MethodCall(_) | syn::Expr::Try(_)
    )
}

impl Detector for MissingErrorPropagation {
    fn name(&self) -> &str {
        "missing-error-propagation"
    }

    fn description(&self) -> &str {
        "Detects silently discarded Result values via `let _ = expr`"
    }

    fn severity(&self) -> Severity {
        Severity::Low
    }

    fn confidence(&self) -> Confidence {
        Confidence::High
    }

    fn detect(&self, ctx: &AnalysisContext) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (path, ast) in ctx.raw_asts() {
            let mut searcher = WildcardLetSearcher {
                findings: Vec::new(),
            };
            syn::visit::visit_file(&mut searcher, ast);

            for (line, col) in &searcher.findings {
                findings.push(Finding {
                    detector_name: self.name().to_string(),
                    title: "Silently discarded Result value".to_string(),
                    description:
                        "A function or method call result is discarded with `let _ = ...`. \
                         If the call returns a Result, errors will be silently ignored."
                            .to_string(),
                    severity: Severity::Low,
                    confidence: Confidence::High,
                    locations: vec![SourceLocation {
                        file: path.clone(),
                        start_line: *line,
                        end_line: *line,
                        start_col: *col,
                        end_col: *col,
                        snippet: None,
                    }],
                    recommendation: Some(
                        "Handle the error with `?` or explicitly ignore with `.ok()`."
                            .to_string(),
                    ),
                    fix: Some(FixSuggestion {
                        description: "Add `.ok()` to explicitly acknowledge the discarded Result"
                            .to_string(),
                        replacement_text: "let _ = /* expr */.ok();".to_string(),
                        location: SourceLocation {
                            file: path.clone(),
                            start_line: *line,
                            end_line: *line,
                            start_col: *col,
                            end_col: *col,
                            snippet: None,
                        },
                    }),
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
        MissingErrorPropagation.detect(&ctx)
    }

    #[test]
    fn test_detects_discarded_result() {
        let source = r#"
            fn save(deps: DepsMut) {
                let _ = CONFIG.save(deps.storage, &config);
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "missing-error-propagation");
    }

    #[test]
    fn test_no_finding_with_propagation() {
        let source = r#"
            fn save(deps: DepsMut) -> StdResult<()> {
                CONFIG.save(deps.storage, &config)?;
                Ok(())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_no_finding_for_literal_wildcard() {
        // `let _ = 42` is not a call, should not flag
        let source = r#"
            fn ignore() {
                let _ = 42;
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
