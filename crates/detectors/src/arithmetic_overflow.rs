use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects wrapping arithmetic operations on cosmwasm-std Int types.
/// CWA-2024-002: Int.neg() uses wrapping math which can silently overflow.
pub struct ArithmeticOverflow;

const WRAPPING_METHODS: &[&str] = &[
    "neg",
    "wrapping_add",
    "wrapping_sub",
    "wrapping_mul",
    "overflowing_add",
    "overflowing_sub",
    "overflowing_mul",
];

struct OverflowSearcher {
    findings: Vec<(usize, usize, String)>,
}

impl<'ast> Visit<'ast> for OverflowSearcher {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if WRAPPING_METHODS.contains(&method.as_str()) {
            let span = node.method.span();
            self.findings
                .push((span.start().line, span.start().column, method));
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

impl Detector for ArithmeticOverflow {
    fn name(&self) -> &str {
        "arithmetic-overflow"
    }

    fn description(&self) -> &str {
        "Detects wrapping arithmetic that can silently overflow (CWA-2024-002)"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn confidence(&self) -> Confidence {
        Confidence::Medium
    }

    fn detect(&self, ctx: &AnalysisContext) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (path, ast) in ctx.raw_asts() {
            let mut searcher = OverflowSearcher {
                findings: Vec::new(),
            };
            syn::visit::visit_file(&mut searcher, ast);

            for (line, col, method) in &searcher.findings {
                findings.push(Finding {
                    detector_name: self.name().to_string(),
                    title: format!("Potential arithmetic overflow via .{}()", method),
                    description: format!(
                        "Method `.{}()` uses wrapping arithmetic which can silently \
                         overflow. On cosmwasm-std Int types this can produce incorrect \
                         values without error.",
                        method
                    ),
                    severity: Severity::High,
                    confidence: Confidence::Medium,
                    locations: vec![SourceLocation {
                        file: path.clone(),
                        start_line: *line,
                        end_line: *line,
                        start_col: *col,
                        end_col: *col,
                        snippet: None,
                    }],
                    recommendation: Some(format!(
                        "Use checked arithmetic (e.g. `.checked_{}()`) instead.",
                        method.strip_prefix("wrapping_").unwrap_or(method)
                    )),
                    fix: None,
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
        ArithmeticOverflow.detect(&ctx)
    }

    #[test]
    fn test_detects_neg() {
        let source = r#"
            fn negate(val: Int128) -> Int128 {
                val.neg()
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "arithmetic-overflow");
    }

    #[test]
    fn test_no_finding_checked_ops() {
        let source = r#"
            fn safe_add(a: Uint128, b: Uint128) -> Option<Uint128> {
                a.checked_add(b)
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_detects_wrapping_add() {
        let source = r#"
            fn wrap(a: u128, b: u128) -> u128 {
                a.wrapping_add(b)
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
    }
}
