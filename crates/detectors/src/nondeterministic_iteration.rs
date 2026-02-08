use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects iteration over HashMap which has nondeterministic order.
/// In CosmWasm, nondeterministic execution can cause consensus failures.
pub struct NondeterministicIteration;

const ITER_METHODS: &[&str] = &["iter", "keys", "values", "into_iter", "drain"];

struct HashMapIterSearcher {
    findings: Vec<(usize, usize)>,
    /// Variable names known to be HashMap from let bindings with type annotations
    hashmap_vars: std::collections::HashSet<String>,
}

impl<'ast> Visit<'ast> for HashMapIterSearcher {
    // Collect variables declared with HashMap type annotations
    fn visit_local(&mut self, node: &'ast syn::Local) {
        if let syn::Pat::Ident(ident) = &node.pat {
            // Check for explicit type annotation containing HashMap
            if let Some(init) = &node.init {
                if expr_mentions_hashmap(&init.expr) {
                    self.hashmap_vars.insert(ident.ident.to_string());
                }
            }
        }
        syn::visit::visit_local(self, node);
    }

    // Also collect function parameters with HashMap type
    fn visit_fn_arg(&mut self, node: &'ast syn::FnArg) {
        if let syn::FnArg::Typed(pat_type) = node {
            if type_mentions_hashmap(&pat_type.ty) {
                if let syn::Pat::Ident(ident) = pat_type.pat.as_ref() {
                    self.hashmap_vars.insert(ident.ident.to_string());
                }
            }
        }
        syn::visit::visit_fn_arg(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if ITER_METHODS.contains(&method.as_str()) && self.receiver_is_hashmap(&node.receiver) {
            let span = node.method.span();
            self.findings.push((span.start().line, span.start().column));
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

impl HashMapIterSearcher {
    /// Check if receiver is a known HashMap variable or contains HashMap in path
    fn receiver_is_hashmap(&self, expr: &syn::Expr) -> bool {
        match expr {
            syn::Expr::Path(path) => {
                if let Some(seg) = path.path.segments.last() {
                    let name = seg.ident.to_string();
                    self.hashmap_vars.contains(&name) || name.contains("HashMap")
                } else {
                    false
                }
            }
            syn::Expr::MethodCall(mc) => self.receiver_is_hashmap(&mc.receiver),
            syn::Expr::Reference(r) => self.receiver_is_hashmap(&r.expr),
            _ => false,
        }
    }
}

fn type_mentions_hashmap(ty: &syn::Type) -> bool {
    if let syn::Type::Path(tp) = ty {
        tp.path
            .segments
            .iter()
            .any(|s| s.ident == "HashMap")
    } else {
        false
    }
}

fn expr_mentions_hashmap(expr: &syn::Expr) -> bool {
    if let syn::Expr::Call(call) = expr {
        if let syn::Expr::Path(path) = call.func.as_ref() {
            return path.path.segments.iter().any(|s| s.ident == "HashMap");
        }
    }
    false
}

impl Detector for NondeterministicIteration {
    fn name(&self) -> &str {
        "nondeterministic-iteration"
    }

    fn description(&self) -> &str {
        "Detects iteration over HashMap with nondeterministic ordering"
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn confidence(&self) -> Confidence {
        Confidence::Medium
    }

    fn detect(&self, ctx: &AnalysisContext) -> Vec<Finding> {
        let mut findings = Vec::new();

        for (path, ast) in ctx.raw_asts() {
            let mut searcher = HashMapIterSearcher {
                findings: Vec::new(),
                hashmap_vars: std::collections::HashSet::new(),
            };
            syn::visit::visit_file(&mut searcher, ast);

            for (line, col) in &searcher.findings {
                findings.push(Finding {
                    detector_name: self.name().to_string(),
                    title: "Nondeterministic iteration over HashMap".to_string(),
                    description:
                        "Iterating over a HashMap produces nondeterministic order. \
                         In CosmWasm, this can cause consensus failures across validators."
                            .to_string(),
                    severity: Severity::Medium,
                    confidence: Confidence::Medium,
                    locations: vec![SourceLocation {
                        file: path.clone(),
                        start_line: *line,
                        end_line: *line,
                        start_col: *col,
                        end_col: *col,
                        snippet: None,
                    }],
                    recommendation: Some(
                        "Use `BTreeMap` instead, or collect into a Vec and sort.".to_string(),
                    ),
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
        NondeterministicIteration.detect(&ctx)
    }

    #[test]
    fn test_detects_hashmap_iter() {
        let source = r#"
            fn process(data: HashMap<String, u128>) {
                for (k, v) in data.iter() {
                    do_something(k, v);
                }
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "nondeterministic-iteration");
    }

    #[test]
    fn test_no_finding_btreemap() {
        let source = r#"
            fn process(data: BTreeMap<String, u128>) {
                for (k, v) in data.iter() {
                    do_something(k, v);
                }
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_no_finding_vec_iter() {
        let source = r#"
            fn process(data: Vec<u128>) {
                for v in data.iter() {
                    do_something(v);
                }
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
