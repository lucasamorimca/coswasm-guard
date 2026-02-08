use std::path::{Path, PathBuf};

use anyhow::Result;

use cosmwasm_guard::ast::analyze_crate;
use cosmwasm_guard::detector::{AnalysisContext, DetectorRegistry};
use cosmwasm_guard::finding::Severity;
use cosmwasm_guard::ir::builder::IrBuilder;
use cosmwasm_guard::report::AnalysisReport;

use crate::output;
use crate::{OutputFormat, SeverityFilter};

pub fn run(
    path: &Path,
    format: OutputFormat,
    severity: SeverityFilter,
    detectors: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    quiet: bool,
    no_color: bool,
) -> Result<()> {
    // 1. Parse and merge all .rs files into a single ContractInfo (crate-level analysis)
    let (contract, source_map) = analyze_crate(path)?;
    let files: Vec<PathBuf> = source_map.keys().cloned().collect();

    if !quiet {
        eprintln!("Analyzing {} files...", files.len());
    }

    // 2. Build detector registry
    let mut all_dets = cosmwasm_guard_detectors::all_detectors();

    if let Some(ref names) = detectors {
        all_dets.retain(|d| names.iter().any(|n| n == d.name()));
    }
    if let Some(ref names) = exclude {
        all_dets.retain(|d| !names.iter().any(|n| n == d.name()));
    }

    let mut registry = DetectorRegistry::new();
    registry.register_all(all_dets);

    // 3. Build IR from merged contract and run detectors
    let ir = IrBuilder::build_contract(&contract);
    let ctx = AnalysisContext::new(&contract, &ir, &source_map);
    let mut all_findings = registry.run_all(&ctx);

    // Enrich findings with source snippets
    for finding in &mut all_findings {
        for loc in &mut finding.locations {
            if loc.snippet.is_none() {
                if let Some(source) = source_map.get(&loc.file) {
                    loc.snippet = get_snippet(source, loc.start_line, loc.end_line);
                }
            }
        }
    }

    // 4. Filter by severity
    let min_severity = match severity {
        SeverityFilter::High => Severity::High,
        SeverityFilter::Medium => Severity::Medium,
        SeverityFilter::Low => Severity::Low,
        SeverityFilter::Info => Severity::Informational,
    };
    all_findings.retain(|f| f.severity <= min_severity);

    // 5. Build report
    let report = AnalysisReport::from_findings(files, all_findings);

    // 6. Output
    match format {
        OutputFormat::Json => output::json::print(&report)?,
        OutputFormat::Sarif => output::sarif::print(&report)?,
        OutputFormat::Text => output::text::print(&report, quiet, no_color)?,
    }

    // 7. Exit code
    if report.total_findings > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn get_snippet(source: &str, start_line: usize, end_line: usize) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let start = start_line.saturating_sub(1);
    // Use end_line inclusive (end_line is 1-based, convert to 0-based exclusive)
    let end = end_line.min(lines.len());
    if start >= lines.len() {
        return None;
    }
    Some(lines[start..end].join("\n"))
}
