use std::path::{Path, PathBuf};

use anyhow::Result;

use cosmwasm_guard::ast::analyze_crate;
use cosmwasm_guard::config::{self, Config};
use cosmwasm_guard::detector::{AnalysisContext, DetectorRegistry};
use cosmwasm_guard::finding::Severity;
use cosmwasm_guard::ir::builder::IrBuilder;
use cosmwasm_guard::report::AnalysisReport;

use crate::output;
use crate::{OutputFormat, SeverityFilter};

#[allow(clippy::too_many_arguments)]
pub fn run(
    path: &Path,
    format: OutputFormat,
    severity: SeverityFilter,
    detectors: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    config_path: Option<PathBuf>,
    audit: bool,
    quiet: bool,
    no_color: bool,
) -> Result<()> {
    // 1. Load config
    let config_file = config_path.unwrap_or_else(|| PathBuf::from(".cosmwasm-guard.toml"));
    let config = Config::load(&config_file)?;

    // 2. Parse and merge all .rs files into a single ContractInfo (crate-level analysis)
    let (contract, source_map) = analyze_crate(path)?;
    let files: Vec<PathBuf> = source_map.keys().cloned().collect();

    if !quiet {
        eprintln!("Analyzing {} files...", files.len());
    }

    // 3. Build detector registry
    let mut all_dets = cosmwasm_guard_detectors::all_detectors();

    // Apply config-based detector filtering
    all_dets.retain(|d| config.is_detector_enabled(d.name()));

    if let Some(ref names) = detectors {
        all_dets.retain(|d| names.iter().any(|n| n == d.name()));
    }
    if let Some(ref names) = exclude {
        all_dets.retain(|d| !names.iter().any(|n| n == d.name()));
    }

    let mut registry = DetectorRegistry::new();
    registry.register_all(all_dets);

    // 4. Build IR from merged contract and run detectors
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

    // 5. Apply inline suppressions
    let inline_suppressions = config::parse_inline_suppressions(&source_map);
    all_findings = config::apply_suppressions(all_findings, &config, &inline_suppressions);

    // 6. Filter by severity (CLI flag overrides config, audit mode lowers to informational)
    let min_severity = if audit {
        Severity::Informational
    } else {
        match severity {
            SeverityFilter::High => Severity::High,
            SeverityFilter::Medium => Severity::Medium,
            SeverityFilter::Low => Severity::Low,
            SeverityFilter::Info => Severity::Informational,
        }
    };
    all_findings.retain(|f| f.severity <= min_severity);

    // 7. Build report
    let report = AnalysisReport::from_findings(files, all_findings);

    // 8. Output
    match format {
        OutputFormat::Json => output::json::print(&report)?,
        OutputFormat::Sarif => output::sarif::print(&report)?,
        OutputFormat::Text => output::text::print(&report, quiet, no_color)?,
    }

    // 9. Exit code
    if report.total_findings > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn get_snippet(source: &str, start_line: usize, end_line: usize) -> Option<String> {
    let lines: Vec<&str> = source.lines().collect();
    let start = start_line.saturating_sub(1);
    let end = end_line.min(lines.len());
    if start >= lines.len() {
        return None;
    }
    Some(lines[start..end].join("\n"))
}
