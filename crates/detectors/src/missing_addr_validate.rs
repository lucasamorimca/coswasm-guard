use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;
use syn::visit::Visit;

/// Detects string addresses in message types that are not validated with addr_validate()
pub struct MissingAddrValidate;

/// Address-like field name patterns
const ADDRESS_PATTERNS: &[&str] = &[
    "addr",
    "address",
    "sender",
    "recipient",
    "owner",
    "admin",
    "operator",
    "minter",
    "beneficiary",
    "delegate",
    "guardian",
];

fn is_address_field_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    ADDRESS_PATTERNS.iter().any(|p| lower.contains(p))
}

/// Visitor that searches function bodies for addr_validate calls on a specific field
struct AddrValidateSearcher {
    field_name: String,
    found: bool,
}

impl<'ast> Visit<'ast> for AddrValidateSearcher {
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        let method_name = node.method.to_string();
        if method_name == "addr_validate" || method_name == "addr_canonicalize" {
            // Check if any argument references the field we're tracking
            for arg in &node.args {
                if expr_references_name(arg, &self.field_name) {
                    self.found = true;
                    return;
                }
            }
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

/// Check if an expression references a variable name (simple heuristic)
fn expr_references_name(expr: &syn::Expr, name: &str) -> bool {
    match expr {
        syn::Expr::Path(p) => p.path.segments.last().is_some_and(|s| s.ident == name),
        syn::Expr::Reference(r) => expr_references_name(&r.expr, name),
        syn::Expr::Field(f) => {
            if let syn::Member::Named(ident) = &f.member {
                ident == name
            } else {
                expr_references_name(&f.base, name)
            }
        }
        _ => false,
    }
}

impl Detector for MissingAddrValidate {
    fn name(&self) -> &str {
        "missing-addr-validate"
    }

    fn description(&self) -> &str {
        "Detects string addresses in message types not validated with addr_validate()"
    }

    fn severity(&self) -> Severity {
        Severity::Medium
    }

    fn confidence(&self) -> Confidence {
        Confidence::Medium
    }

    fn detect(&self, ctx: &AnalysisContext) -> Vec<Finding> {
        let mut findings = Vec::new();

        // Find String fields with address-like names in message enums
        for msg_enum in &ctx.contract.message_enums {
            for variant in &msg_enum.variants {
                for field in &variant.fields {
                    if field.type_name == "String" && is_address_field_name(&field.name) {
                        // Check if any function body validates this field
                        let validated = self.is_field_validated(ctx, &field.name);
                        if !validated {
                            findings.push(Finding {
                                detector_name: self.name().to_string(),
                                title: format!(
                                    "Unvalidated address: `{}` in {}::{}",
                                    field.name, msg_enum.name, variant.name
                                ),
                                description: format!(
                                    "Field `{}` of type String in {}: {} looks like an address \
                                     but is never passed to addr_validate(). Unvalidated addresses \
                                     can cause funds to be sent to invalid or unreachable addresses.",
                                    field.name, msg_enum.name, variant.name
                                ),
                                severity: Severity::Medium,
                                confidence: Confidence::Medium,
                                locations: vec![SourceLocation {
                                    file: msg_enum.span.file.clone(),
                                    start_line: msg_enum.span.start_line,
                                    end_line: msg_enum.span.end_line,
                                    start_col: msg_enum.span.start_col,
                                    end_col: msg_enum.span.end_col,
                                    snippet: None,
                                }],
                                recommendation: Some(format!(
                                    "Validate the address with `deps.api.addr_validate(&{})?;`",
                                    field.name
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

impl MissingAddrValidate {
    /// Check if a field name is validated with addr_validate in any function body
    fn is_field_validated(&self, ctx: &AnalysisContext, field_name: &str) -> bool {
        for (_path, ast) in ctx.raw_asts() {
            let mut searcher = AddrValidateSearcher {
                field_name: field_name.to_string(),
                found: false,
            };
            syn::visit::visit_file(&mut searcher, ast);
            if searcher.found {
                return true;
            }
        }
        false
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
        MissingAddrValidate.detect(&ctx)
    }

    #[test]
    fn test_detects_unvalidated_address() {
        let source = r#"
            pub enum ExecuteMsg {
                Transfer { recipient: String, amount: u128 },
            }
            #[entry_point]
            pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                match msg {
                    ExecuteMsg::Transfer { recipient, amount } => {
                        Ok(Response::new())
                    }
                }
            }
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "missing-addr-validate");
    }

    #[test]
    fn test_no_finding_when_validated() {
        let source = r#"
            pub enum ExecuteMsg {
                Transfer { recipient: String },
            }
            #[entry_point]
            pub fn execute(deps: DepsMut, _env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                match msg {
                    ExecuteMsg::Transfer { recipient } => {
                        let validated = deps.api.addr_validate(&recipient)?;
                        Ok(Response::new())
                    }
                }
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_ignores_non_address_string_fields() {
        let source = r#"
            pub enum ExecuteMsg {
                SetName { name: String },
            }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
