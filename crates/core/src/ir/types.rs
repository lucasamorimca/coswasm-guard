use crate::ast::SourceSpan;

use super::cfg::Cfg;
use super::instruction::SsaVar;

/// IR representation of an entire contract
#[derive(Debug)]
pub struct ContractIr {
    pub functions: Vec<FunctionIr>,
    pub entry_points: Vec<String>,
}

impl ContractIr {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            entry_points: Vec::new(),
        }
    }

    /// Get IR for a specific function by name
    pub fn get_function(&self, name: &str) -> Option<&FunctionIr> {
        self.functions.iter().find(|f| f.name == name)
    }

    /// Get all entry point function IRs
    pub fn entry_point_functions(&self) -> Vec<&FunctionIr> {
        self.functions.iter().filter(|f| f.is_entry_point).collect()
    }
}

impl Default for ContractIr {
    fn default() -> Self {
        Self::new()
    }
}

/// IR for a single function
#[derive(Debug)]
pub struct FunctionIr {
    pub name: String,
    pub params: Vec<SsaVar>,
    pub cfg: Cfg,
    pub is_entry_point: bool,
    pub source_span: SourceSpan,
}
