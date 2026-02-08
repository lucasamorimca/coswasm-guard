use std::path::Path;

use super::contract_info::{EntryPointKind, SourceSpan, StorageType};

/// Extract a type name from a syn::Type as a string
pub fn type_to_string(ty: &syn::Type) -> String {
    quote::quote!(#ty).to_string().replace(' ', "")
}

/// Convert a proc_macro2::Span to our SourceSpan
pub fn span_to_source_span(span: proc_macro2::Span, file: &Path) -> SourceSpan {
    SourceSpan {
        file: file.to_path_buf(),
        start_line: span.start().line,
        end_line: span.end().line,
        start_col: span.start().column,
        end_col: span.end().column,
    }
}

/// Check if an attribute is #[entry_point]
pub fn is_entry_point_attr(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("entry_point")
}

/// Infer entry point kind from function name
pub fn infer_entry_point_kind(fn_name: &str) -> EntryPointKind {
    match fn_name {
        "instantiate" => EntryPointKind::Instantiate,
        "execute" => EntryPointKind::Execute,
        "query" => EntryPointKind::Query,
        "migrate" => EntryPointKind::Migrate,
        "sudo" => EntryPointKind::Sudo,
        "reply" => EntryPointKind::Reply,
        _ => EntryPointKind::Unknown,
    }
}

/// Fallback: infer entry point kind from parameter types when name is Unknown.
/// Uses CosmWasm signature conventions (msg type name + deps mutability).
pub fn infer_entry_point_kind_from_params(params: &[super::contract_info::ParamInfo]) -> EntryPointKind {
    let type_names: Vec<&str> = params.iter().map(|p| p.type_name.as_str()).collect();

    // Check for Reply type (unique parameter)
    if type_names.contains(&"Reply") {
        return EntryPointKind::Reply;
    }

    let has_deps_mut = type_names.iter().any(|t| t.contains("DepsMut"));
    let has_message_info = type_names.iter().any(|t| t.contains("MessageInfo"));

    // Infer from msg type name if present (last param is typically the msg)
    if let Some(last) = params.last() {
        let msg_type = &last.type_name;
        if msg_type.contains("Instantiate") {
            return EntryPointKind::Instantiate;
        }
        if msg_type.contains("Execute") {
            return EntryPointKind::Execute;
        }
        if msg_type.contains("Query") {
            return EntryPointKind::Query;
        }
        if msg_type.contains("Migrate") {
            return EntryPointKind::Migrate;
        }
        if msg_type.contains("Sudo") {
            return EntryPointKind::Sudo;
        }
    }

    // Fallback: use deps mutability + MessageInfo presence
    if has_message_info && has_deps_mut {
        // Could be Execute or Instantiate — can't distinguish without msg type
        return EntryPointKind::Execute;
    }
    if !has_deps_mut {
        return EntryPointKind::Query;
    }

    EntryPointKind::Unknown
}

/// Check if a type path matches a cw-storage-plus storage type
pub fn detect_storage_type(path: &syn::Path) -> Option<StorageType> {
    let last_segment = path.segments.last()?;
    match last_segment.ident.to_string().as_str() {
        "Item" => Some(StorageType::Item),
        "Map" => Some(StorageType::Map),
        "IndexedMap" => Some(StorageType::IndexedMap),
        _ => None,
    }
}

/// Extract generic type arguments from a path segment as strings.
/// e.g., `Item<Config>` -> ["Config"], `Map<&str, Uint128>` -> ["&str", "Uint128"]
pub fn extract_generic_args(path: &syn::Path) -> Vec<String> {
    let Some(last_segment) = path.segments.last() else {
        return Vec::new();
    };
    match &last_segment.arguments {
        syn::PathArguments::AngleBracketed(args) => args
            .args
            .iter()
            .filter_map(|arg| match arg {
                syn::GenericArgument::Type(ty) => Some(type_to_string(ty)),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Check if a message kind can be inferred from enum name
pub fn infer_message_kind(enum_name: &str) -> super::contract_info::MessageKind {
    use super::contract_info::MessageKind;
    if enum_name.contains("Instantiate") || enum_name == "InstantiateMsg" {
        MessageKind::Instantiate
    } else if enum_name.contains("Execute") || enum_name == "ExecuteMsg" {
        MessageKind::Execute
    } else if enum_name.contains("Query") || enum_name == "QueryMsg" {
        MessageKind::Query
    } else if enum_name.contains("Migrate") || enum_name == "MigrateMsg" {
        MessageKind::Migrate
    } else {
        MessageKind::Unknown
    }
}
