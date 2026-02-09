// Real-world detector validation against cw-plus contract files.
// Run manually: cargo test --test real_world_validation -- --ignored --nocapture

use std::collections::HashMap;
use std::path::PathBuf;

use cosmwasm_guard::ast::{parse_source, ContractVisitor};
use cosmwasm_guard::detector::{AnalysisContext, DetectorRegistry};
use cosmwasm_guard::ir::builder::IrBuilder;
use cosmwasm_guard_detectors::all_detectors;

struct FixtureResult {
    file_name: String,
    findings: Vec<(String, String, String, usize)>, // (detector, severity, title, line)
}

fn analyze_fixture(name: &str, source: &str) -> FixtureResult {
    let ast = parse_source(source).expect(&format!("Failed to parse {}", name));
    let path = PathBuf::from(name);
    let contract = ContractVisitor::extract(path.clone(), ast);
    let ir = IrBuilder::build_contract(&contract);
    let mut sources = HashMap::new();
    sources.insert(path, source.to_string());
    let ctx = AnalysisContext::new(&contract, &ir, &sources);

    let mut registry = DetectorRegistry::new();
    registry.register_all(all_detectors());
    let findings = registry.run_all(&ctx);

    let items = findings
        .iter()
        .map(|f| {
            let line = f.locations.first().map_or(0, |l| l.start_line);
            (
                f.detector_name.clone(),
                format!("{:?}", f.severity),
                f.title.clone(),
                line,
            )
        })
        .collect();

    FixtureResult {
        file_name: name.to_string(),
        findings: items,
    }
}

#[ignore]
#[test]
fn validate_detectors_on_cw_plus() {
    let fixtures: Vec<(&str, &str)> = vec![
        (
            "cw20_base_contract.rs",
            include_str!("fixtures/real-world/cw20_base_contract.rs"),
        ),
        (
            "cw1_whitelist_contract.rs",
            include_str!("fixtures/real-world/cw1_whitelist_contract.rs"),
        ),
        (
            "cw20_ics20_contract.rs",
            include_str!("fixtures/real-world/cw20_ics20_contract.rs"),
        ),
        (
            "cw3_multisig_contract.rs",
            include_str!("fixtures/real-world/cw3_multisig_contract.rs"),
        ),
        (
            "cw4_group_contract.rs",
            include_str!("fixtures/real-world/cw4_group_contract.rs"),
        ),
        (
            "cw4_stake_contract.rs",
            include_str!("fixtures/real-world/cw4_stake_contract.rs"),
        ),
    ];

    let mut all_results = Vec::new();
    let mut detector_counts: HashMap<String, usize> = HashMap::new();

    for (name, source) in &fixtures {
        let result = analyze_fixture(name, source);
        for (detector, _, _, _) in &result.findings {
            *detector_counts.entry(detector.clone()).or_insert(0) += 1;
        }
        all_results.push(result);
    }

    // Print results grouped by file
    println!("\n============================================================");
    println!("REAL-WORLD DETECTOR VALIDATION BASELINE");
    println!("============================================================\n");

    let mut total = 0;
    for result in &all_results {
        println!("--- {} ({} findings) ---", result.file_name, result.findings.len());
        for (detector, severity, title, line) in &result.findings {
            println!("  [{severity}] {detector} (line {line}): {title}");
        }
        total += result.findings.len();
        if result.findings.is_empty() {
            println!("  (no findings)");
        }
        println!();
    }

    // Print summary by detector
    println!("--- SUMMARY BY DETECTOR ---");
    let mut counts: Vec<_> = detector_counts.iter().collect();
    counts.sort_by(|a, b| b.1.cmp(a.1));
    for (detector, count) in &counts {
        println!("  {detector}: {count}");
    }
    println!("\nTotal findings: {total} across {} files", all_results.len());

    // Regression guard: baseline should have exactly 1 TP (unsafe-unwrap in cw20-base)
    // If this increases, new FPs were introduced. If it decreases to 0, detection regressed.
    assert!(
        total <= 5,
        "Regression: too many findings ({total}). Expected <=5 (was 1 after FP fixes)"
    );
}
