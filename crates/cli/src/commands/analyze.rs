use std::path::{Path, PathBuf};

use anyhow::Result;

use cosmwasm_guard::ast::analyze_crate_cached;
use cosmwasm_guard::cache::CacheManager;
use cosmwasm_guard::config::{self, Config};
use cosmwasm_guard::detector::{AnalysisContext, DetectorRegistry};
use cosmwasm_guard::finding::Severity;
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
    no_cache: bool,
    quiet: bool,
    no_color: bool,
) -> Result<()> {
    // 1. Load config
    let config_file = config_path.unwrap_or_else(|| PathBuf::from(".cosmwasm-guard.toml"));
    let config = Config::load(&config_file)?;

    // 2. Set up optional cache
    let mut cache = if no_cache {
        None
    } else {
        let cache_dir = path.join(".cosmwasm-guard-cache");
        CacheManager::open(cache_dir).ok()
    };

    // 3. Parse, merge, and build IR (with caching when enabled)
    let analysis = analyze_crate_cached(path, cache.as_mut())?;
    let files: Vec<PathBuf> = analysis.source_map.keys().cloned().collect();

    if !quiet {
        eprintln!("Analyzing {} files...", files.len());
    }

    // 4. Build detector registry
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

    // 5. Run detectors (parallel when >= 4 detectors)
    let ctx = AnalysisContext::new(&analysis.contract, &analysis.ir, &analysis.source_map);
    let mut all_findings = registry.run_all(&ctx);

    // Enrich findings with source snippets
    for finding in &mut all_findings {
        for loc in &mut finding.locations {
            if loc.snippet.is_none() {
                if let Some(source) = analysis.source_map.get(&loc.file) {
                    loc.snippet = get_snippet(source, loc.start_line, loc.end_line);
                }
            }
        }
    }

    // 6. Apply inline suppressions
    let inline_suppressions = config::parse_inline_suppressions(&analysis.source_map);
    all_findings = config::apply_suppressions(all_findings, &config, &inline_suppressions);

    // 7. Filter by severity (CLI flag overrides config, audit mode lowers to informational)
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

    // 8. Build report
    let report = AnalysisReport::from_findings(files, all_findings);

    // 9. Output
    match format {
        OutputFormat::Json => output::json::print(&report)?,
        OutputFormat::Sarif => output::sarif::print(&report)?,
        OutputFormat::Text => output::text::print(&report, quiet, no_color)?,
    }

    // 10. Exit code
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
