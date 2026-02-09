use cosmwasm_guard::ast::{EntryPointKind, FunctionInfo};
use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects execute handlers without info.sender authorization checks.
/// Follows dispatch patterns: if execute() delegates to handler functions
/// via match arms, checks those handlers for sender checks too.
pub struct MissingAccessControl;

/// Visitor that searches for info.sender usage in expressions
struct SenderCheckSearcher {
    found_sender_check: bool,
}

impl<'ast> Visit<'ast> for SenderCheckSearcher {
    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if let syn::Member::Named(ident) = &node.member {
            if ident == "sender" && is_info_expr(&node.base) {
                self.found_sender_check = true;
            }
        }
        syn::visit::visit_expr_field(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        // Recognize ecosystem access-control helpers:
        // assert_owner(), is_owner(), cw_ownable::assert_owner(), etc.
        if let syn::Expr::Path(path) = node.func.as_ref() {
            let full_path = path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            let last_segment = path.path.segments.last().map(|s| s.ident.to_string());
            if let Some(name) = last_segment {
                if name == "assert_owner"
                    || name == "is_owner"
                    || name == "check_owner"
                    || name == "validate_owner"
                    || full_path.contains("cw_ownable")
                {
                    self.found_sender_check = true;
                }
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method = node.method.to_string();
        if method == "assert_owner"
            || method == "is_owner"
            || method == "check_owner"
            || method == "validate_owner"
        {
            self.found_sender_check = true;
        }
        syn::visit::visit_expr_method_call(self, node);
    }

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
            let tokens = node.tokens.to_string();
            // Direct sender check: ensure_eq!(info.sender, ...)
            if tokens.contains("info") && tokens.contains("sender") {
                self.found_sender_check = true;
            }
            // Owner/admin pattern with sender context:
            // ensure_eq!(info.sender, owner, ...) or ensure!(is_admin(&info), ...)
            if (tokens.contains("owner") || tokens.contains("admin"))
                && (tokens.contains("info") || tokens.contains("sender"))
            {
                self.found_sender_check = true;
            }
        }
    }
}

/// Visitor that extracts function call names from match arm bodies.
/// Used to find dispatch patterns like `match msg { Variant => handler_fn(deps, ...) }`.
struct DispatchCallCollector {
    called_functions: Vec<String>,
}

impl<'ast> Visit<'ast> for DispatchCallCollector {
    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = node.func.as_ref() {
            if let Some(last) = path.path.segments.last() {
                self.called_functions.push(last.ident.to_string());
            }
        }
        syn::visit::visit_expr_call(self, node);
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

/// Check if a function body has an info.sender check
fn has_sender_check(body: &syn::Block) -> bool {
    let mut searcher = SenderCheckSearcher {
        found_sender_check: false,
    };
    syn::visit::visit_block(&mut searcher, body);
    searcher.found_sender_check
}

/// Extract function names called from match arms in a block (dispatch pattern)
fn extract_dispatched_functions(body: &syn::Block) -> Vec<String> {
    let mut collector = DispatchCallCollector {
        called_functions: Vec::new(),
    };
    // Only look inside match expressions at the top level of the block
    for stmt in &body.stmts {
        if let syn::Stmt::Expr(syn::Expr::Match(m), _) = stmt {
            for arm in &m.arms {
                syn::visit::visit_expr(&mut collector, &arm.body);
            }
        }
    }
    collector.called_functions
}

/// Check if dispatched handler functions have sender checks
fn handlers_have_sender_checks(
    dispatched_fns: &[String],
    all_functions: &[FunctionInfo],
) -> bool {
    if dispatched_fns.is_empty() {
        return false;
    }
    // At least one dispatched handler must check info.sender
    dispatched_fns.iter().any(|fn_name| {
        all_functions
            .iter()
            .find(|f| f.name == *fn_name)
            .and_then(|f| f.body.as_ref())
            .is_some_and(has_sender_check)
    })
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

        for ep in &ctx.contract.entry_points {
            if ep.kind != EntryPointKind::Execute {
                continue;
            }

            let func = ctx.contract.functions.iter().find(|f| f.name == ep.name);
            let Some(func) = func else { continue };
            let Some(body) = &func.body else { continue };

            // Direct check: does the execute function body itself check info.sender?
            if has_sender_check(body) {
                continue;
            }

            // Dispatch following: does execute() delegate to handler functions
            // that check info.sender?
            let dispatched = extract_dispatched_functions(body);
            if handlers_have_sender_checks(&dispatched, &ctx.contract.functions) {
                continue;
            }

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
                fix: None,
            });
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

    #[test]
    fn test_no_finding_with_assert_owner() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                assert_owner(deps.storage, &info.sender)?;
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty(), "assert_owner() should count as access control");
    }

    #[test]
    fn test_no_finding_with_cw_ownable_call() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                cw_ownable::assert_owner(deps.storage, &info.sender)?;
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty(), "cw_ownable::assert_owner() should count as access control");
    }

    #[test]
    fn test_no_finding_with_ensure_eq_owner() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                let owner = OWNER.load(deps.storage)?;
                ensure_eq!(info.sender, owner, ContractError::Unauthorized);
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty(), "ensure_eq! with owner should count as access control");
    }

    // --- H6 regression: dispatch following through match arms ---

    #[test]
    fn test_h6_dispatch_to_handler_with_sender_check() {
        // execute() dispatches to handler_transfer() which checks info.sender
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                match msg {
                    ExecuteMsg::Transfer { recipient } => {
                        handler_transfer(deps, env, info, recipient)
                    }
                }
            }

            fn handler_transfer(deps: DepsMut, env: Env, info: MessageInfo, recipient: String)
                -> StdResult<Response> {
                if info.sender != owner {
                    return Err(StdError::generic_err("unauthorized"));
                }
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(
            findings.is_empty(),
            "H6: dispatch to handler with sender check should not flag"
        );
    }

    #[test]
    fn test_h6_dispatch_to_handler_without_sender_check() {
        // execute() dispatches to handler without any sender check
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                match msg {
                    ExecuteMsg::Withdraw {} => handle_withdraw(deps),
                }
            }

            fn handle_withdraw(deps: DepsMut) -> StdResult<Response> {
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(
            !findings.is_empty(),
            "H6: dispatch to handler without sender check should still flag"
        );
    }
}
