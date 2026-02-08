use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use super::contract_info::ContractInfo;
use super::visitor::ContractVisitor;

/// Analyze an entire CosmWasm crate (all .rs files) and merge into single ContractInfo
pub fn analyze_crate(
    crate_path: &Path,
) -> Result<(ContractInfo, std::collections::HashMap<PathBuf, String>)> {
    let rs_files = discover_rs_files(crate_path)?;
    let mut merged = ContractInfo::new(crate_path.to_path_buf());
    let mut source_map = std::collections::HashMap::new();

    for file_path in &rs_files {
        let source = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read: {}", file_path.display()))?;

        let ast = syn::parse_file(&source)
            .with_context(|| format!("Failed to parse: {}", file_path.display()))?;

        let mut visitor = ContractVisitor::new(file_path.clone());
        syn::visit::visit_file(&mut visitor, &ast);

        merged.merge_from_visitor(
            visitor.entry_points,
            visitor.message_enums,
            visitor.state_items,
            visitor.functions,
            file_path.clone(),
            ast,
        );
        source_map.insert(file_path.clone(), source);
    }

    Ok((merged, source_map))
}

/// Discover all .rs files in a crate directory
fn discover_rs_files(path: &Path) -> Result<Vec<PathBuf>> {
    // If path is a single file, return it directly
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    // Look for src/ directory
    let src_dir = path.join("src");
    let search_dir = if src_dir.exists() { &src_dir } else { path };

    let files: Vec<PathBuf> = WalkDir::new(search_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "rs"))
        .filter(|e| !e.path().to_string_lossy().contains("/target/"))
        .map(|e| e.path().to_path_buf())
        .collect();

    if files.is_empty() {
        anyhow::bail!("No .rs files found in: {}", path.display());
    }

    Ok(files)
}
