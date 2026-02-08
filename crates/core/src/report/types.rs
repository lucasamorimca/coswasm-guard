use std::path::PathBuf;

use serde::Serialize;

use crate::finding::{Finding, Severity};

#[derive(Debug, Serialize)]
pub struct SeverityCounts {
    pub high: usize,
    pub medium: usize,
    pub low: usize,
    pub informational: usize,
}

#[derive(Debug, Serialize)]
pub struct AnalysisReport {
    pub files_analyzed: Vec<PathBuf>,
    pub total_findings: usize,
    pub findings_by_severity: SeverityCounts,
    pub findings: Vec<Finding>,
}

impl AnalysisReport {
    pub fn from_findings(files: Vec<PathBuf>, findings: Vec<Finding>) -> Self {
        let counts = SeverityCounts {
            high: findings
                .iter()
                .filter(|f| f.severity == Severity::High)
                .count(),
            medium: findings
                .iter()
                .filter(|f| f.severity == Severity::Medium)
                .count(),
            low: findings
                .iter()
                .filter(|f| f.severity == Severity::Low)
                .count(),
            informational: findings
                .iter()
                .filter(|f| f.severity == Severity::Informational)
                .count(),
        };
        let total = findings.len();
        Self {
            files_analyzed: files,
            total_findings: total,
            findings_by_severity: counts,
            findings,
        }
    }
}
