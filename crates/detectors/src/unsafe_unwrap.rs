use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects .unwrap() and .expect() calls in non-test contract code.
/// Panics in CosmWasm contracts cause chain-halting errors.
pub struct UnsafeUnwrap;

struct UnwrapSearcher {
    findings: Vec<(usize, usize, String)>, // (line, col, method)
    in_test_module: bool,
}

impl<'ast> Visit<'ast> for UnwrapSearcher {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        // Skip #[cfg(test)] modules
        let is_test = node.attrs.iter().any(|attr| {
            if attr.path().is_ident("cfg") {
                attr.meta.require_list().ok().is_some_and(|list| {
                    list.tokens.to_string().contains("test")
                })
            } else {
                false
            }
        });
        if is_test {
            return;
        }
        syn::visit::visit_item_mod(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if self.in_test_module {
            syn::visit::visit_expr_method_call(self, node);
            return;
        }
        let method = node.method.to_string();
        if method == "unwrap" || method == "expect" {
            let span = node.method.span();
            self.findings
                .push((span.start().line, span.start().column, method));
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

impl Detector for UnsafeUnwrap {
    fn name(&self) -> &str {
        "unsafe-unwrap"
    }

    fn description(&self) -> &str {
        "Detects .unwrap() and .expect() calls that can panic in contract code"
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn confidence(&self) -> Confidence {
        Confidence::High
    }

    fn detect(&self, ctx: &AnalysisContext) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (path, ast) in ctx.raw_asts() {
            let mut searcher = UnwrapSearcher {
                findings: Vec::new(),
                in_test_module: false,
            };
            syn::visit::visit_file(&mut searcher, ast);

            for (line, col, method) in &searcher.findings {
                findings.push(Finding {
                    detector_name: self.name().to_string(),
                    title: format!("Unsafe .{}() call", method),
                    description: format!(
                        "Calling .{}() can panic and halt the chain. \
                         Use the `?` operator or explicit error handling instead.",
                        method
                    ),
                    severity: Severity::Medium,
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
                        "Replace `.unwrap()` with `?` or handle the error explicitly."
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
        UnsafeUnwrap.detect(&ctx)
    }

    #[test]
    fn test_detects_unwrap() {
        let source = r#"
            fn load_config(deps: Deps) -> Config {
                CONFIG.load(deps.storage).unwrap()
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "unsafe-unwrap");
    }

    #[test]
    fn test_no_finding_with_question_mark() {
        let source = r#"
            fn load_config(deps: Deps) -> StdResult<Config> {
                let config = CONFIG.load(deps.storage)?;
                Ok(config)
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_skips_test_modules() {
        let source = r#"
            #[cfg(test)]
            mod tests {
                fn test_helper() {
                    let x = Some(1).unwrap();
                }
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
