use std::path::PathBuf;

use syn::visit::Visit;

use super::contract_info::*;
use super::utils;

/// AST visitor that extracts CosmWasm contract information from a parsed file
pub struct ContractVisitor {
    file_path: PathBuf,
    pub entry_points: Vec<EntryPoint>,
    pub message_enums: Vec<MessageEnum>,
    pub state_items: Vec<StateItem>,
    pub functions: Vec<FunctionInfo>,
}

impl ContractVisitor {
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            entry_points: Vec::new(),
            message_enums: Vec::new(),
            state_items: Vec::new(),
            functions: Vec::new(),
        }
    }

    /// Parse and visit a file, returning a single-file ContractInfo.
    /// Takes ownership of `ast` to avoid cloning the entire syn::File tree.
    pub fn extract(file_path: PathBuf, ast: syn::File) -> ContractInfo {
        let mut visitor = ContractVisitor::new(file_path.clone());
        syn::visit::visit_file(&mut visitor, &ast);

        let mut info = ContractInfo::new(file_path.clone());
        info.merge_from_visitor(
            visitor.entry_points,
            visitor.message_enums,
            visitor.state_items,
            visitor.functions,
            file_path,
            ast,
        );
        info
    }
}

impl<'ast> Visit<'ast> for ContractVisitor {
    /// Visit function items — detect #[entry_point] and collect all functions
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        let fn_name = node.sig.ident.to_string();
        let span = utils::span_to_source_span(node.sig.ident.span(), &self.file_path);

        // Extract parameters
        let params: Vec<ParamInfo> = node
            .sig
            .inputs
            .iter()
            .filter_map(|arg| {
                if let syn::FnArg::Typed(pat_type) = arg {
                    let name = quote::quote!(#pat_type.pat).to_string();
                    let name = if let syn::Pat::Ident(ident) = pat_type.pat.as_ref() {
                        ident.ident.to_string()
                    } else {
                        name
                    };
                    let type_name = utils::type_to_string(&pat_type.ty);
                    Some(ParamInfo { name, type_name })
                } else {
                    None
                }
            })
            .collect();

        // Check for #[entry_point] attribute
        let is_entry_point = node.attrs.iter().any(utils::is_entry_point_attr);

        if is_entry_point {
            let mut kind = utils::infer_entry_point_kind(&fn_name);
            if kind == EntryPointKind::Unknown {
                kind = utils::infer_entry_point_kind_from_params(&params);
            }
            let has_deps_mut = params.iter().any(|p| p.type_name.contains("DepsMut"));

            self.entry_points.push(EntryPoint {
                name: fn_name.clone(),
                kind,
                params: params.clone(),
                span: span.clone(),
                has_deps_mut,
            });
        }

        // Collect as generic function info
        let return_type = match &node.sig.output {
            syn::ReturnType::Default => None,
            syn::ReturnType::Type(_, ty) => Some(utils::type_to_string(ty)),
        };

        self.functions.push(FunctionInfo {
            name: fn_name,
            params,
            return_type,
            span,
            body: Some((*node.block).clone()),
        });

        syn::visit::visit_item_fn(self, node);
    }

    /// Visit enum items — detect ExecuteMsg, QueryMsg, etc.
    fn visit_item_enum(&mut self, node: &'ast syn::ItemEnum) {
        let enum_name = node.ident.to_string();

        // Only capture enums with "Msg" suffix or known message names
        if !enum_name.ends_with("Msg") && !enum_name.ends_with("Message") {
            syn::visit::visit_item_enum(self, node);
            return;
        }

        let kind = utils::infer_message_kind(&enum_name);
        let span = utils::span_to_source_span(node.ident.span(), &self.file_path);

        let variants: Vec<MessageVariant> = node
            .variants
            .iter()
            .map(|v| {
                let fields: Vec<FieldInfo> = match &v.fields {
                    syn::Fields::Named(named) => named
                        .named
                        .iter()
                        .map(|f| FieldInfo {
                            name: f.ident.as_ref().map_or_else(String::new, |i| i.to_string()),
                            type_name: utils::type_to_string(&f.ty),
                        })
                        .collect(),
                    syn::Fields::Unnamed(unnamed) => unnamed
                        .unnamed
                        .iter()
                        .enumerate()
                        .map(|(i, f)| FieldInfo {
                            name: format!("_{i}"),
                            type_name: utils::type_to_string(&f.ty),
                        })
                        .collect(),
                    syn::Fields::Unit => Vec::new(),
                };
                MessageVariant {
                    name: v.ident.to_string(),
                    fields,
                }
            })
            .collect();

        self.message_enums.push(MessageEnum {
            name: enum_name,
            kind,
            variants,
            span,
        });

        syn::visit::visit_item_enum(self, node);
    }

    /// Visit const items — detect Item<T> and Map<K,V> storage declarations
    fn visit_item_const(&mut self, node: &'ast syn::ItemConst) {
        // Check if type is Item<_>, Map<_, _>, or IndexedMap<_, _>
        if let syn::Type::Path(type_path) = node.ty.as_ref() {
            if let Some(storage_type) = utils::detect_storage_type(&type_path.path) {
                let const_name = node.ident.to_string();
                let span = utils::span_to_source_span(node.ident.span(), &self.file_path);
                let generic_args = utils::extract_generic_args(&type_path.path);

                let (key_type, value_type) = match storage_type {
                    StorageType::Item => (None, generic_args.first().cloned().unwrap_or_default()),
                    StorageType::Map | StorageType::IndexedMap => {
                        let key = generic_args.first().cloned();
                        let val = generic_args.get(1).cloned().unwrap_or_default();
                        (key, val)
                    }
                };

                // Try to extract storage key from constructor arg: Item::new("key")
                let storage_key = extract_storage_key_from_expr(&node.expr);

                self.state_items.push(StateItem {
                    name: const_name,
                    storage_type,
                    key_type,
                    value_type,
                    storage_key,
                    span,
                });
            }
        }

        syn::visit::visit_item_const(self, node);
    }

    /// Visit impl blocks — collect methods as FunctionInfo
    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        for item in &node.items {
            if let syn::ImplItem::Fn(method) = item {
                let fn_name = method.sig.ident.to_string();
                let span = utils::span_to_source_span(method.sig.ident.span(), &self.file_path);

                let params: Vec<ParamInfo> = method
                    .sig
                    .inputs
                    .iter()
                    .filter_map(|arg| {
                        if let syn::FnArg::Typed(pat_type) = arg {
                            let name = if let syn::Pat::Ident(ident) = pat_type.pat.as_ref() {
                                ident.ident.to_string()
                            } else {
                                "_".to_string()
                            };
                            let type_name = utils::type_to_string(&pat_type.ty);
                            Some(ParamInfo { name, type_name })
                        } else {
                            None
                        }
                    })
                    .collect();

                let return_type = match &method.sig.output {
                    syn::ReturnType::Default => None,
                    syn::ReturnType::Type(_, ty) => Some(utils::type_to_string(ty)),
                };

                self.functions.push(FunctionInfo {
                    name: fn_name,
                    params,
                    return_type,
                    span,
                    body: Some(method.block.clone()),
                });
            }
        }

        syn::visit::visit_item_impl(self, node);
    }
}

/// Try to extract a string literal storage key from a constructor expression
/// e.g., `Item::new("config")` -> Some("config")
fn extract_storage_key_from_expr(expr: &syn::Expr) -> Option<String> {
    if let syn::Expr::Call(call) = expr {
        // Look for the first string literal argument
        for arg in &call.args {
            if let syn::Expr::Lit(lit) = arg {
                if let syn::Lit::Str(s) = &lit.lit {
                    return Some(s.value());
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::parse_source;

    fn parse_and_visit(source: &str) -> ContractInfo {
        let ast = parse_source(source).unwrap();
        ContractVisitor::extract(PathBuf::from("test.rs"), ast)
    }

    #[test]
    fn test_detect_entry_point() {
        let source = r#"
            #[entry_point]
            pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> Result<Response, ContractError> {
                Ok(Response::new())
            }
        "#;
        let info = parse_and_visit(source);
        assert_eq!(info.entry_points.len(), 1);
        assert_eq!(info.entry_points[0].kind, EntryPointKind::Execute);
        assert_eq!(info.entry_points[0].name, "execute");
        assert!(info.entry_points[0].has_deps_mut);
    }

    #[test]
    fn test_detect_execute_msg_enum() {
        let source = r#"
            pub enum ExecuteMsg {
                Transfer { recipient: String, amount: Uint128 },
                Withdraw {},
            }
        "#;
        let info = parse_and_visit(source);
        assert_eq!(info.message_enums.len(), 1);
        assert_eq!(info.message_enums[0].name, "ExecuteMsg");
        assert_eq!(info.message_enums[0].variants.len(), 2);
        assert_eq!(info.message_enums[0].variants[0].name, "Transfer");
        assert_eq!(info.message_enums[0].variants[0].fields.len(), 2);
    }

    #[test]
    fn test_detect_state_items() {
        let source = r#"
            const CONFIG: Item<Config> = Item::new("config");
            const BALANCES: Map<&str, Uint128> = Map::new("balances");
        "#;
        let info = parse_and_visit(source);
        assert_eq!(info.state_items.len(), 2);
        assert_eq!(info.state_items[0].name, "CONFIG");
        assert_eq!(info.state_items[0].storage_type, StorageType::Item);
        assert_eq!(info.state_items[0].value_type, "Config");
        assert_eq!(info.state_items[0].storage_key, Some("config".to_string()));
        assert_eq!(info.state_items[1].name, "BALANCES");
        assert_eq!(info.state_items[1].storage_type, StorageType::Map);
    }

    #[test]
    fn test_no_entry_points() {
        let source = "fn helper() -> u32 { 42 }";
        let info = parse_and_visit(source);
        assert!(info.entry_points.is_empty());
        assert_eq!(info.functions.len(), 1);
    }

    #[test]
    fn test_query_entry_point_has_deps_not_mut() {
        let source = r#"
            #[entry_point]
            pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
                Ok(Binary::default())
            }
        "#;
        let info = parse_and_visit(source);
        assert_eq!(info.entry_points.len(), 1);
        assert_eq!(info.entry_points[0].kind, EntryPointKind::Query);
        assert!(!info.entry_points[0].has_deps_mut);
    }

    // --- M2 regression: renamed entry points infer kind from param types ---

    #[test]
    fn test_m2_renamed_execute_entry_point() {
        // Renamed function with #[entry_point] should infer Execute from msg type
        let source = r#"
            #[entry_point]
            pub fn handle_exec(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg)
                -> StdResult<Response> {
                Ok(Response::new())
            }
        "#;
        let info = parse_and_visit(source);
        assert_eq!(info.entry_points.len(), 1);
        assert_eq!(info.entry_points[0].name, "handle_exec");
        assert_eq!(info.entry_points[0].kind, EntryPointKind::Execute);
    }

    #[test]
    fn test_m2_renamed_query_entry_point() {
        // Renamed query function should infer Query from msg type
        let source = r#"
            #[entry_point]
            pub fn my_query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
                Ok(Binary::default())
            }
        "#;
        let info = parse_and_visit(source);
        assert_eq!(info.entry_points.len(), 1);
        assert_eq!(info.entry_points[0].kind, EntryPointKind::Query);
    }
}
