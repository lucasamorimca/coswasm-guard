use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use walkdir::WalkDir;

use super::contract_info::ContractInfo;
use super::visitor::ContractVisitor;
use crate::cache::{CacheManager, CachedFileArtifact};
use crate::ir::builder::IrBuilder;
use crate::ir::types::ContractIr;

/// Result of analyzing a crate: contract info, IR, and source map
pub struct CrateAnalysis {
    pub contract: ContractInfo,
    pub ir: ContractIr,
    pub source_map: std::collections::HashMap<PathBuf, String>,
}

/// Analyze an entire CosmWasm crate with optional file-level caching.
/// Returns merged ContractInfo, ContractIr, and source map.
pub fn analyze_crate_cached(
    crate_path: &Path,
    mut cache: Option<&mut CacheManager>,
) -> Result<CrateAnalysis> {
    let rs_files = discover_rs_files(crate_path)?;
    let mut merged = ContractInfo::new(crate_path.to_path_buf());
    let mut ir = ContractIr::new();
    let mut source_map = std::collections::HashMap::new();

    for file_path in &rs_files {
        let source = std::fs::read_to_string(file_path)
            .with_context(|| format!("Failed to read: {}", file_path.display()))?;
        let hash = CacheManager::hash_contents(&source);

        // Parse once — used for raw_asts AND visitor/cache
        let ast = syn::parse_file(&source)
            .with_context(|| format!("Failed to parse: {}", file_path.display()))?;

        // Try cache lookup
        let cached = cache
            .as_deref()
            .and_then(|c| c.lookup(file_path, &hash));

        if let Some(artifact) = cached {
            // Cache hit — merge cached data (skips visitor + IR build)
            CacheManager::merge_cached_into(&artifact, &mut merged, &mut ir, file_path.clone());

            // Re-visit AST to populate FunctionInfo.body fields (not serializable,
            // but detectors need them for pattern matching)
            let mut visitor = ContractVisitor::new(file_path.clone());
            syn::visit::visit_file(&mut visitor, &ast);
            repopulate_function_bodies(&mut merged, &visitor);

            // Push raw AST for detectors
            merged.raw_asts.push((file_path.clone(), ast));
        } else {
            // Cache miss — full visitor + IR build
            let mut visitor = ContractVisitor::new(file_path.clone());
            syn::visit::visit_file(&mut visitor, &ast);

            // Build per-file IR
            let file_contract = build_file_contract(file_path, &visitor);
            let file_ir = IrBuilder::build_contract(&file_contract);

            // Store to cache
            if let Some(ref mut c) = cache {
                let artifact = CachedFileArtifact {
                    entry_points: visitor.entry_points.clone(),
                    message_enums: visitor.message_enums.clone(),
                    state_items: visitor.state_items.clone(),
                    functions: visitor.functions.clone(),
                    ir_functions: file_ir.functions.clone(),
                    ir_entry_points: file_ir.entry_points.clone(),
                };
                // Non-fatal: log but don't fail on cache write errors
                let _ = c.store(file_path, &hash, &artifact);
            }

            // Merge into main structures
            merged.merge_from_visitor(
                visitor.entry_points,
                visitor.message_enums,
                visitor.state_items,
                visitor.functions,
                file_path.clone(),
                ast,
            );
            ir.functions.extend(file_ir.functions);
            for ep in file_ir.entry_points {
                if !ir.entry_points.contains(&ep) {
                    ir.entry_points.push(ep);
                }
            }
        }

        source_map.insert(file_path.clone(), source);
    }

    // Fix up entry point flags on IR functions (cached files may not know about
    // entry points from other files)
    let ep_names: Vec<String> = merged.entry_points.iter().map(|ep| ep.name.clone()).collect();
    ir.entry_points = ep_names.clone();
    for func in &mut ir.functions {
        func.is_entry_point = ep_names.contains(&func.name);
    }

    // Flush cache manifest
    if let Some(c) = cache {
        let _ = c.flush();
    }

    Ok(CrateAnalysis {
        contract: merged,
        ir,
        source_map,
    })
}

/// On cache hit, FunctionInfo.body is None (not serializable). Re-populate
/// by matching function names from a fresh visitor pass.
fn repopulate_function_bodies(merged: &mut ContractInfo, visitor: &ContractVisitor) {
    for func in &mut merged.functions {
        if func.body.is_none() {
            if let Some(fresh) = visitor.functions.iter().find(|f| f.name == func.name) {
                func.body = fresh.body.clone();
            }
        }
    }
}

/// Build a temporary single-file ContractInfo for per-file IR building
fn build_file_contract(file_path: &Path, visitor: &ContractVisitor) -> ContractInfo {
    let mut ci = ContractInfo::new(file_path.to_path_buf());
    ci.entry_points = visitor.entry_points.clone();
    ci.functions = visitor.functions.clone();
    ci
}

/// Original non-cached interface (backwards compatible)
pub fn analyze_crate(
    crate_path: &Path,
) -> Result<(ContractInfo, std::collections::HashMap<PathBuf, String>)> {
    let result = analyze_crate_cached(crate_path, None)?;
    Ok((result.contract, result.source_map))
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
