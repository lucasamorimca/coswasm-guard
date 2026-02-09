use std::collections::HashSet;

use cosmwasm_guard::ast::EntryPointKind;
use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects state items loaded in execute/query that are never saved in instantiate.
/// Accessing uninitialized state can panic or return unexpected defaults.
pub struct UninitializedStateAccess;

impl Detector for UninitializedStateAccess {
    fn name(&self) -> &str {
        "uninitialized-state-access"
    }

    fn description(&self) -> &str {
        "Detects state loaded in handlers but never initialized in instantiate"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn confidence(&self) -> Confidence {
        Confidence::Medium
    }

    fn detect(&self, ctx: &AnalysisContext) -> Vec<Finding> {
        // Collect state item names from contract declarations
        let state_names: HashSet<String> = ctx
            .contract
            .state_items
            .iter()
            .map(|s| s.name.clone())
            .collect();

        if state_names.is_empty() {
            return Vec::new();
        }

        // Find which state items are saved/updated in instantiate handlers
        let mut initialized_in_instantiate: HashSet<String> = HashSet::new();
        for ep in &ctx.contract.entry_points {
            if ep.kind != EntryPointKind::Instantiate {
                continue;
            }
            if let Some(func) = ctx.contract.functions.iter().find(|f| f.name == ep.name) {
                if let Some(body) = &func.body {
                    let saves = collect_save_calls(body);
                    initialized_in_instantiate.extend(saves);
                }
            }
        }

        // Find state items loaded in execute/query but not initialized
        let mut findings = Vec::new();
        for ep in &ctx.contract.entry_points {
            if ep.kind != EntryPointKind::Execute && ep.kind != EntryPointKind::Query {
                continue;
            }
            if let Some(func) = ctx.contract.functions.iter().find(|f| f.name == ep.name) {
                if let Some(body) = &func.body {
                    let loads = collect_load_calls(body);
                    for (name, line, col) in loads {
                        if state_names.contains(&name)
                            && !initialized_in_instantiate.contains(&name)
                        {
                            findings.push(Finding {
                                detector_name: self.name().to_string(),
                                title: format!(
                                    "State `{}` loaded but may not be initialized",
                                    name
                                ),
                                description: format!(
                                    "`{}` is loaded in `{}` but is never saved in any \
                                     instantiate handler. This will panic with a `NotFound` \
                                     error on first access.",
                                    name, ep.name
                                ),
                                severity: Severity::High,
                                confidence: Confidence::Medium,
                                locations: vec![SourceLocation {
                                    file: ep.span.file.clone(),
                                    start_line: line,
                                    end_line: line,
                                    start_col: col,
                                    end_col: col,
                                    snippet: None,
                                }],
                                recommendation: Some(format!(
                                    "Ensure `{}.save(...)` is called in the instantiate handler, \
                                     or use `.may_load()` with a default value.",
                                    name
                                )),
                                fix: None,
                            });
                        }
                    }
                }
            }
        }

        findings
    }
}

/// Collect names of state items that have .save() or .update() called on them
fn collect_save_calls(block: &syn::Block) -> HashSet<String> {
    struct SaveSearcher {
        saves: HashSet<String>,
    }

    impl<'ast> Visit<'ast> for SaveSearcher {
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            let method = node.method.to_string();
            if method == "save" || method == "update" {
                if let Some(name) = extract_receiver_name(&node.receiver) {
                    self.saves.insert(name);
                }
            }
            syn::visit::visit_expr_method_call(self, node);
        }
    }

    let mut searcher = SaveSearcher {
        saves: HashSet::new(),
    };
    syn::visit::visit_block(&mut searcher, block);
    searcher.saves
}

/// Collect (name, line, col) of state items that have .load() called on them
fn collect_load_calls(block: &syn::Block) -> Vec<(String, usize, usize)> {
    struct LoadSearcher {
        loads: Vec<(String, usize, usize)>,
    }

    impl<'ast> Visit<'ast> for LoadSearcher {
        fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
            let method = node.method.to_string();
            if method == "load" {
                if let Some(name) = extract_receiver_name(&node.receiver) {
                    let span = node.method.span();
                    self.loads
                        .push((name, span.start().line, span.start().column));
                }
            }
            syn::visit::visit_expr_method_call(self, node);
        }
    }

    let mut searcher = LoadSearcher { loads: Vec::new() };
    syn::visit::visit_block(&mut searcher, block);
    searcher.loads
}

/// Extract the receiver name from a method call (e.g., `CONFIG` from `CONFIG.load(...)`)
fn extract_receiver_name(expr: &syn::Expr) -> Option<String> {
    match expr {
        syn::Expr::Path(path) => {
            let name = path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            Some(name)
        }
        _ => None,
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
        UninitializedStateAccess.detect(&ctx)
    }

    #[test]
    fn test_detects_uninitialized_load() {
        let source = r#"
            use cw_storage_plus::Item;
            pub const CONFIG: Item<Config> = Item::new("config");

            #[entry_point]
            pub fn instantiate(deps: DepsMut, env: Env, info: MessageInfo, msg: InstantiateMsg)
                -> Result<Response, ContractError> {
                Ok(Response::new())
            }

            #[entry_point]
            pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> Result<Response, ContractError> {
                let config = CONFIG.load(deps.storage)?;
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "uninitialized-state-access");
    }

    #[test]
    fn test_no_finding_when_initialized() {
        let source = r#"
            use cw_storage_plus::Item;
            pub const CONFIG: Item<Config> = Item::new("config");

            #[entry_point]
            pub fn instantiate(deps: DepsMut, env: Env, info: MessageInfo, msg: InstantiateMsg)
                -> Result<Response, ContractError> {
                CONFIG.save(deps.storage, &Config::default())?;
                Ok(Response::new())
            }

            #[entry_point]
            pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> Result<Response, ContractError> {
                let config = CONFIG.load(deps.storage)?;
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_no_finding_with_may_load() {
        // may_load() returns Option — safe on uninitialized state, should NOT flag
        let source = r#"
            use cw_storage_plus::Item;
            pub const CONFIG: Item<Config> = Item::new("config");

            #[entry_point]
            pub fn instantiate(deps: DepsMut, env: Env, info: MessageInfo, msg: InstantiateMsg)
                -> Result<Response, ContractError> {
                Ok(Response::new())
            }

            #[entry_point]
            pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> Result<Response, ContractError> {
                let config = CONFIG.may_load(deps.storage)?;
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty(), "may_load() should not be flagged as uninitialized access");
    }

    #[test]
    fn test_no_finding_without_state_items() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> Result<Response, ContractError> {
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
