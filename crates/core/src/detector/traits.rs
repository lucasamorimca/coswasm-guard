use super::context::AnalysisContext;
use crate::finding::{Confidence, Finding, Severity};

/// Core trait for all vulnerability detectors.
/// Implementors analyze a CosmWasm contract and return findings.
pub trait Detector: Send + Sync {
    /// Unique identifier for this detector (e.g., "missing-addr-validate")
    fn name(&self) -> &str;

    /// Human-readable description of what this detector checks
    fn description(&self) -> &str;

    /// Default severity of findings from this detector
    fn severity(&self) -> Severity;

    /// Default confidence level of findings from this detector
    fn confidence(&self) -> Confidence;

    /// Run detection on the given analysis context, return findings
    fn detect(&self, context: &AnalysisContext) -> Vec<Finding>;
}
