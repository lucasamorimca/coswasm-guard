use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects Map::range() calls without .take() limits, risking gas exhaustion
pub struct UnboundedIteration;

/// Visitor that finds .range() calls and checks for .take() in the method chain
struct RangeCallSearcher {
    unbounded_ranges: Vec<UnboundedRange>,
    file_path: std::path::PathBuf,
}

struct UnboundedRange {
    line: usize,
    col: usize,
}

impl<'ast> Visit<'ast> for RangeCallSearcher {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();

        // We look for method chains ending in .collect(), .for_each(), etc.
        // that contain .range() but not .take()
        if is_terminal_method(&method) {
            let chain = collect_method_chain(node);
            let has_range = chain.iter().any(|m| m == "range" || m == "range_raw");
            let has_take = chain.iter().any(|m| m == "take");

            if has_range && !has_take {
                let span = node.method.span();
                self.unbounded_ranges.push(UnboundedRange {
                    line: span.start().line,
                    col: span.start().column,
                });
            }
        }

        syn::visit::visit_expr_method_call(self, node);
    }
}

fn is_terminal_method(method: &str) -> bool {
    matches!(
        method,
        "collect" | "for_each" | "count" | "sum" | "fold" | "last" | "max" | "min"
    )
}

/// Walk up the method call chain and collect method names
fn collect_method_chain(node: &syn::ExprMethodCall) -> Vec<String> {
    let mut methods = vec![node.method.to_string()];
    let mut current: &syn::Expr = &node.receiver;

    while let syn::Expr::MethodCall(mc) = current {
        methods.push(mc.method.to_string());
        current = &mc.receiver;
    }

    methods.reverse();
    methods
}

impl Detector for UnboundedIteration {
    fn name(&self) -> &str {
        "unbounded-iteration"
    }

    fn description(&self) -> &str {
        "Detects Map::range() calls without .take() limits risking gas exhaustion"
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
            let mut searcher = RangeCallSearcher {
                unbounded_ranges: Vec::new(),
                file_path: path.clone(),
            };
            syn::visit::visit_file(&mut searcher, ast);

            for range_call in &searcher.unbounded_ranges {
                findings.push(Finding {
                    detector_name: self.name().to_string(),
                    title: "Unbounded iteration over storage Map".to_string(),
                    description:
                        "A .range() call on a storage Map does not include a .take() limit. \
                         If the map grows large, iterating without a limit will exhaust gas \
                         and cause the transaction to fail."
                            .to_string(),
                    severity: Severity::Medium,
                    confidence: Confidence::High,
                    locations: vec![SourceLocation {
                        file: searcher.file_path.clone(),
                        start_line: range_call.line,
                        end_line: range_call.line,
                        start_col: range_call.col,
                        end_col: range_call.col,
                        snippet: None,
                    }],
                    recommendation: Some(
                        "Add `.take(limit)` after `.range()` to bound iteration, e.g.: \
                         `MAP.range(storage, None, None, Order::Ascending).take(100)`"
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
        UnboundedIteration.detect(&ctx)
    }

    #[test]
    fn test_detects_unbounded_range() {
        let source = r#"
            fn list_all(deps: Deps) -> Vec<(String, u128)> {
                BALANCES
                    .range(deps.storage, None, None, Order::Ascending)
                    .collect::<StdResult<Vec<_>>>()
                    .unwrap()
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "unbounded-iteration");
    }

    #[test]
    fn test_no_finding_with_take() {
        let source = r#"
            fn list_limited(deps: Deps, limit: usize) -> Vec<(String, u128)> {
                BALANCES
                    .range(deps.storage, None, None, Order::Ascending)
                    .take(limit)
                    .collect::<StdResult<Vec<_>>>()
                    .unwrap()
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_no_finding_without_range() {
        let source = r#"
            fn get_one(deps: Deps) -> u128 {
                BALANCES.load(deps.storage, "alice").unwrap()
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
