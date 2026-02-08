use std::collections::HashMap;
use std::path::PathBuf;

use cosmwasm_guard::ast::{parse_source, ContractVisitor};
use cosmwasm_guard::detector::{AnalysisContext, DetectorRegistry};
use cosmwasm_guard::ir::builder::IrBuilder;
use cosmwasm_guard_detectors::all_detectors;

fn analyze_source(source: &str) -> Vec<cosmwasm_guard::finding::Finding> {
    let ast = parse_source(source).unwrap();
    let contract = ContractVisitor::extract(PathBuf::from("test.rs"), ast);
    let ir = IrBuilder::build_contract(&contract);
    let mut sources = HashMap::new();
    sources.insert(PathBuf::from("test.rs"), source.to_string());
    let ctx = AnalysisContext::new(&contract, &ir, &sources);

    let mut registry = DetectorRegistry::new();
    registry.register_all(all_detectors());
    registry.run_all(&ctx)
}

#[test]
fn test_vulnerable_contract_has_findings() {
    let source = include_str!("fixtures/vulnerable_contract.rs");
    let findings = analyze_source(source);

    // Should detect: missing addr_validate, missing access control, unbounded iteration
    assert!(
        findings.len() >= 3,
        "Expected at least 3 findings, got {}",
        findings.len()
    );

    let detector_names: Vec<&str> = findings.iter().map(|f| f.detector_name.as_str()).collect();
    assert!(
        detector_names.contains(&"missing-addr-validate"),
        "missing-addr-validate not found in {:?}",
        detector_names
    );
    assert!(
        detector_names.contains(&"missing-access-control"),
        "missing-access-control not found in {:?}",
        detector_names
    );
    assert!(
        detector_names.contains(&"unbounded-iteration"),
        "unbounded-iteration not found in {:?}",
        detector_names
    );
}

#[test]
fn test_safe_contract_no_findings() {
    let source = include_str!("fixtures/safe_contract.rs");
    let findings = analyze_source(source);

    assert!(
        findings.is_empty(),
        "Safe contract should have no findings, got: {:?}",
        findings
            .iter()
            .map(|f| &f.detector_name)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_severity_ordering() {
    let source = include_str!("fixtures/vulnerable_contract.rs");
    let findings = analyze_source(source);

    // Findings should be sorted by severity (High first)
    let severities: Vec<_> = findings.iter().map(|f| &f.severity).collect();
    for window in severities.windows(2) {
        assert!(window[0] <= window[1], "Findings not sorted by severity");
    }
}
