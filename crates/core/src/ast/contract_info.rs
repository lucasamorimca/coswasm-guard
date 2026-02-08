use std::path::PathBuf;

use serde::Serialize;

/// Source location in a file
#[derive(Debug, Clone, Serialize)]
pub struct SourceSpan {
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

/// Parameter info for functions/entry points
#[derive(Debug, Clone, Serialize)]
pub struct ParamInfo {
    pub name: String,
    pub type_name: String,
}

/// Kind of CosmWasm entry point
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum EntryPointKind {
    Instantiate,
    Execute,
    Query,
    Migrate,
    Sudo,
    Reply,
    Unknown,
}

/// A #[entry_point] function
#[derive(Debug, Clone, Serialize)]
pub struct EntryPoint {
    pub name: String,
    pub kind: EntryPointKind,
    pub params: Vec<ParamInfo>,
    pub span: SourceSpan,
    pub has_deps_mut: bool,
}

/// Message enum kind
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum MessageKind {
    Instantiate,
    Execute,
    Query,
    Migrate,
    Unknown,
}

/// A field in a message variant
#[derive(Debug, Clone, Serialize)]
pub struct FieldInfo {
    pub name: String,
    pub type_name: String,
}

/// A variant in a message enum
#[derive(Debug, Clone, Serialize)]
pub struct MessageVariant {
    pub name: String,
    pub fields: Vec<FieldInfo>,
}

/// A message enum (ExecuteMsg, QueryMsg, etc.)
#[derive(Debug, Clone, Serialize)]
pub struct MessageEnum {
    pub name: String,
    pub kind: MessageKind,
    pub variants: Vec<MessageVariant>,
    pub span: SourceSpan,
}

/// Storage type (cw-storage-plus)
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum StorageType {
    Item,
    Map,
    IndexedMap,
}

/// A state storage declaration
#[derive(Debug, Clone, Serialize)]
pub struct StateItem {
    pub name: String,
    pub storage_type: StorageType,
    pub key_type: Option<String>,
    pub value_type: String,
    pub storage_key: Option<String>,
    pub span: SourceSpan,
}

/// Generic function info
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    pub name: String,
    pub params: Vec<ParamInfo>,
    pub return_type: Option<String>,
    pub span: SourceSpan,
    pub body: Option<syn::Block>,
}

/// Top-level container for parsed CosmWasm contract information.
/// For multi-file crates, this merges data from all source files.
#[derive(Debug)]
pub struct ContractInfo {
    pub crate_path: PathBuf,
    pub source_files: Vec<PathBuf>,
    pub entry_points: Vec<EntryPoint>,
    pub message_enums: Vec<MessageEnum>,
    pub state_items: Vec<StateItem>,
    pub functions: Vec<FunctionInfo>,
    pub raw_asts: Vec<(PathBuf, syn::File)>,
}

impl ContractInfo {
    pub fn new(crate_path: PathBuf) -> Self {
        Self {
            crate_path,
            source_files: Vec::new(),
            entry_points: Vec::new(),
            message_enums: Vec::new(),
            state_items: Vec::new(),
            functions: Vec::new(),
            raw_asts: Vec::new(),
        }
    }

    /// Merge results from a visitor into this ContractInfo
    pub fn merge_from_visitor(
        &mut self,
        entry_points: Vec<EntryPoint>,
        message_enums: Vec<MessageEnum>,
        state_items: Vec<StateItem>,
        functions: Vec<FunctionInfo>,
        file_path: PathBuf,
        ast: syn::File,
    ) {
        self.source_files.push(file_path.clone());
        self.entry_points.extend(entry_points);
        self.message_enums.extend(message_enums);
        self.state_items.extend(state_items);
        self.functions.extend(functions);
        self.raw_asts.push((file_path, ast));
    }
}
