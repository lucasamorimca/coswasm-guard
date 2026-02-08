use cosmwasm_guard::ast::EntryPointKind;
use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects reply handlers that don't validate msg.id, risking
/// processing the wrong submessage reply.
pub struct SubmessageReplyUnvalidated;

/// Checks if a function body references `msg.id` or `reply.id`
struct ReplyIdSearcher {
    found_id_check: bool,
}

impl<'ast> Visit<'ast> for ReplyIdSearcher {
    fn visit_expr_field(&mut self, node: &'ast syn::ExprField) {
        if let syn::Member::Named(ident) = &node.member {
            if ident == "id" {
                self.found_id_check = true;
            }
        }
        syn::visit::visit_expr_field(self, node);
    }

    // Also check match expressions on msg.id
    fn visit_expr_match(&mut self, node: &'ast syn::ExprMatch) {
        if let syn::Expr::Field(field) = node.expr.as_ref() {
            if let syn::Member::Named(ident) = &field.member {
                if ident == "id" {
                    self.found_id_check = true;
                }
            }
        }
        syn::visit::visit_expr_match(self, node);
    }
}

impl Detector for SubmessageReplyUnvalidated {
    fn name(&self) -> &str {
        "submessage-reply-unvalidated"
    }

    fn description(&self) -> &str {
        "Detects reply handlers that don't validate submessage ID"
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
            if ep.kind != EntryPointKind::Reply {
                continue;
            }

            let func = ctx.contract.functions.iter().find(|f| f.name == ep.name);
            let Some(func) = func else { continue };
            let Some(body) = &func.body else { continue };

            let mut searcher = ReplyIdSearcher {
                found_id_check: false,
            };
            syn::visit::visit_block(&mut searcher, body);

            if !searcher.found_id_check {
                findings.push(Finding {
                    detector_name: self.name().to_string(),
                    title: format!("Reply handler `{}` doesn't validate msg.id", ep.name),
                    description: format!(
                        "Reply handler `{}` does not check `msg.id` to identify which \
                         submessage it is responding to. This can cause the handler to \
                         process the wrong reply.",
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
                        "Add `match msg.id { REPLY_ID => ..., id => Err(...) }` \
                         to validate the submessage ID."
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
        SubmessageReplyUnvalidated.detect(&ctx)
    }

    #[test]
    fn test_detects_unvalidated_reply() {
        let source = r#"
            #[entry_point]
            pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "submessage-reply-unvalidated");
    }

    #[test]
    fn test_no_finding_with_id_check() {
        let source = r#"
            #[entry_point]
            pub fn reply(deps: DepsMut, _env: Env, msg: Reply) -> StdResult<Response> {
                match msg.id {
                    1 => Ok(Response::new()),
                    _ => Err(StdError::generic_err("unknown reply")),
                }
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_no_finding_for_non_reply() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                Ok(Response::new())
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
