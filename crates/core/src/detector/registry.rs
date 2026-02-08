use super::context::AnalysisContext;
use super::traits::Detector;
use crate::finding::{Finding, Severity};

/// Registry that holds all detectors and runs them against contracts.
pub struct DetectorRegistry {
    detectors: Vec<Box<dyn Detector>>,
}

impl DetectorRegistry {
    pub fn new() -> Self {
        Self {
            detectors: Vec::new(),
        }
    }

    /// Register a detector
    pub fn register(&mut self, detector: Box<dyn Detector>) {
        self.detectors.push(detector);
    }

    /// Register multiple detectors at once
    pub fn register_all(&mut self, detectors: Vec<Box<dyn Detector>>) {
        self.detectors.extend(detectors);
    }

    /// Run all registered detectors, return aggregated findings sorted by severity
    pub fn run_all(&self, context: &AnalysisContext) -> Vec<Finding> {
        let mut findings: Vec<Finding> = self
            .detectors
            .iter()
            .flat_map(|d| d.detect(context))
            .collect();
        findings.sort_by(|a, b| a.severity.cmp(&b.severity));
        findings
    }

    /// Run only detectors matching the given names
    pub fn run_selected(&self, names: &[&str], context: &AnalysisContext) -> Vec<Finding> {
        let mut findings: Vec<Finding> = self
            .detectors
            .iter()
            .filter(|d| names.contains(&d.name()))
            .flat_map(|d| d.detect(context))
            .collect();
        findings.sort_by(|a, b| a.severity.cmp(&b.severity));
        findings
    }

    /// List all registered detector names
    pub fn list_detectors(&self) -> Vec<&str> {
        self.detectors.iter().map(|d| d.name()).collect()
    }

    /// Filter findings by minimum severity
    pub fn filter_by_severity(findings: Vec<Finding>, min: &Severity) -> Vec<Finding> {
        findings
            .into_iter()
            .filter(|f| f.severity <= *min)
            .collect()
    }
}

impl Default for DetectorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::ContractInfo;
    use crate::finding::*;
    use crate::ir::ContractIr;
    use std::collections::HashMap;
    use std::path::PathBuf;

    struct MockDetector;

    impl Detector for MockDetector {
        fn name(&self) -> &str {
            "mock-detector"
        }
        fn description(&self) -> &str {
            "A mock detector for testing"
        }
        fn severity(&self) -> Severity {
            Severity::Medium
        }
        fn confidence(&self) -> Confidence {
            Confidence::High
        }
        fn detect(&self, _context: &AnalysisContext) -> Vec<Finding> {
            vec![Finding {
                detector_name: "mock-detector".to_string(),
                title: "Mock Finding".to_string(),
                description: "This is a test finding".to_string(),
                severity: Severity::Medium,
                confidence: Confidence::High,
                locations: vec![],
                recommendation: None,
                fix: None,
            }]
        }
    }

    fn make_context() -> (ContractInfo, ContractIr, HashMap<PathBuf, String>) {
        let contract = ContractInfo::new(PathBuf::from("test"));
        let ir = ContractIr::new();
        let sources = HashMap::new();
        (contract, ir, sources)
    }

    #[test]
    fn test_register_and_run() {
        let mut registry = DetectorRegistry::new();
        registry.register(Box::new(MockDetector));

        let (contract, ir, sources) = make_context();
        let ctx = AnalysisContext::new(&contract, &ir, &sources);
        let findings = registry.run_all(&ctx);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].detector_name, "mock-detector");
    }

    #[test]
    fn test_list_detectors() {
        let mut registry = DetectorRegistry::new();
        registry.register(Box::new(MockDetector));
        assert_eq!(registry.list_detectors(), vec!["mock-detector"]);
    }

    #[test]
    fn test_run_selected() {
        let mut registry = DetectorRegistry::new();
        registry.register(Box::new(MockDetector));

        let (contract, ir, sources) = make_context();
        let ctx = AnalysisContext::new(&contract, &ir, &sources);

        let findings = registry.run_selected(&["nonexistent"], &ctx);
        assert!(findings.is_empty());

        let findings = registry.run_selected(&["mock-detector"], &ctx);
        assert_eq!(findings.len(), 1);
    }
}
