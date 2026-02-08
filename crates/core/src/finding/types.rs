use std::path::PathBuf;

use serde::Serialize;

/// Severity levels ordered from most to least severe.
/// IMPORTANT: Variant order matters — derived Ord puts High < Medium < Low < Info,
/// which is used for filtering (retain findings where severity <= threshold).
/// Do NOT reorder these variants.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    High,
    Medium,
    Low,
    Informational,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::High => write!(f, "High"),
            Severity::Medium => write!(f, "Medium"),
            Severity::Low => write!(f, "Low"),
            Severity::Informational => write!(f, "Informational"),
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Confidence {
    High,
    Medium,
    Low,
}

impl std::fmt::Display for Confidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Confidence::High => write!(f, "High"),
            Confidence::Medium => write!(f, "Medium"),
            Confidence::Low => write!(f, "Low"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceLocation {
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub start_col: usize,
    pub end_col: usize,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FixSuggestion {
    pub description: String,
    pub replacement_text: String,
    pub location: SourceLocation,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub detector_name: String,
    pub title: String,
    pub description: String,
    pub severity: Severity,
    pub confidence: Confidence,
    pub locations: Vec<SourceLocation>,
    pub recommendation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<FixSuggestion>,
}
