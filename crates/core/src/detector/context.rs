use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ast::ContractInfo;
use crate::ir::ContractIr;

/// Provides detectors with access to parsed contract info, SSA IR, and source code.
pub struct AnalysisContext<'a> {
    pub contract: &'a ContractInfo,
    pub ir: &'a ContractIr,
    source_files: &'a HashMap<PathBuf, String>,
}

impl<'a> AnalysisContext<'a> {
    pub fn new(
        contract: &'a ContractInfo,
        ir: &'a ContractIr,
        source_files: &'a HashMap<PathBuf, String>,
    ) -> Self {
        Self {
            contract,
            ir,
            source_files,
        }
    }

    /// Get raw ASTs for pattern matching
    pub fn raw_asts(&self) -> &[(PathBuf, syn::File)] {
        &self.contract.raw_asts
    }

    /// Get source code for a specific file
    pub fn source_code(&self, file: &Path) -> Option<&str> {
        self.source_files.get(file).map(|s| s.as_str())
    }

    /// Get source line by file + line number (1-indexed)
    pub fn get_line(&self, file: &Path, line: usize) -> Option<&str> {
        self.source_code(file)?.lines().nth(line.saturating_sub(1))
    }

    /// Extract snippet from a file (start_line and end_line are 1-based inclusive)
    pub fn snippet(&self, file: &Path, start_line: usize, end_line: usize) -> Option<String> {
        let source = self.source_code(file)?;
        let lines: Vec<&str> = source.lines().collect();
        let start = start_line.saturating_sub(1);
        let end = end_line.min(lines.len());
        if start >= lines.len() {
            return None;
        }
        Some(lines[start..end].join("\n"))
    }
}
