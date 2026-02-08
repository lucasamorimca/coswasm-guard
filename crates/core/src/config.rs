use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::finding::{Finding, Severity};

/// Project-level configuration loaded from `.cosmwasm-guard.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub global: GlobalConfig,
    #[serde(default)]
    pub detectors: HashMap<String, DetectorConfig>,
    #[serde(default)]
    pub suppressions: SuppressionConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GlobalConfig {
    pub severity_threshold: String,
    pub output_format: String,
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self {
            severity_threshold: "low".to_string(),
            output_format: "text".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DetectorConfig {
    pub enabled: Option<bool>,
    pub severity: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SuppressionConfig {
    pub files: Vec<String>,
}

impl Config {
    /// Load config from a TOML file path. Returns default config if file doesn't exist.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// Check if a detector is enabled according to config.
    pub fn is_detector_enabled(&self, name: &str) -> bool {
        self.detectors
            .get(name)
            .and_then(|d| d.enabled)
            .unwrap_or(true)
    }

    /// Parse the global severity threshold into a Severity value.
    pub fn severity_threshold(&self) -> Severity {
        parse_severity(&self.global.severity_threshold).unwrap_or(Severity::Low)
    }

    /// Check if a file path should be excluded based on suppression glob patterns.
    pub fn is_file_excluded(&self, file_path: &Path) -> bool {
        let path_str = file_path.to_string_lossy();
        self.suppressions
            .files
            .iter()
            .any(|pattern| glob::Pattern::new(pattern).is_ok_and(|p| p.matches(&path_str)))
    }

    /// Generate default config file content.
    pub fn default_toml() -> &'static str {
        r#"# cosmwasm-guard configuration
# See: https://github.com/safestackai/cosmwasm-guard

[global]
# Minimum severity to report: "high", "medium", "low", "informational"
severity_threshold = "low"
# Output format: "text", "json", "sarif"
output_format = "text"

# Per-detector overrides
# [detectors.unsafe-unwrap]
# enabled = false

# [detectors.missing-addr-validate]
# severity = "low"

[suppressions]
# Glob patterns for files to skip entirely
files = ["tests/**", "examples/**"]
"#
    }
}

fn parse_severity(s: &str) -> Option<Severity> {
    match s.to_lowercase().as_str() {
        "high" => Some(Severity::High),
        "medium" => Some(Severity::Medium),
        "low" => Some(Severity::Low),
        "informational" | "info" => Some(Severity::Informational),
        _ => None,
    }
}

/// Inline suppression: parses source files for `// cosmwasm-guard-ignore` comments.
/// Returns a map of (file, line) → suppressed detector names.
/// A bare `// cosmwasm-guard-ignore` (no colon) suppresses all detectors for that line.
pub fn parse_inline_suppressions(
    source_map: &HashMap<PathBuf, String>,
) -> HashMap<(PathBuf, usize), Vec<String>> {
    let mut suppressions: HashMap<(PathBuf, usize), Vec<String>> = HashMap::new();

    for (path, source) in source_map {
        for (idx, line) in source.lines().enumerate() {
            let trimmed = line.trim();
            if let Some(rest) = extract_suppression_comment(trimmed) {
                // Suppression applies to the *next* line (idx is 0-based, lines are 1-based)
                let target_line = idx + 2;
                let detectors = if rest.is_empty() {
                    vec!["*".to_string()] // wildcard = suppress all
                } else {
                    rest.split(',').map(|s| s.trim().to_string()).collect()
                };
                suppressions.insert((path.clone(), target_line), detectors);
            }
        }
    }

    suppressions
}

/// Extract the detector list from a suppression comment.
/// Returns Some("") for bare ignore, Some("det1, det2") for specific, None if not a suppression.
fn extract_suppression_comment(line: &str) -> Option<&str> {
    // Match: // cosmwasm-guard-ignore or // cosmwasm-guard-ignore: det1, det2
    let comment = line.strip_prefix("//")?;
    let comment = comment.trim();
    let rest = comment.strip_prefix("cosmwasm-guard-ignore")?;
    let rest = rest.trim();
    if rest.is_empty() {
        Some("")
    } else {
        let rest = rest.strip_prefix(':')?;
        Some(rest.trim())
    }
}

/// Filter findings based on config and inline suppressions.
pub fn apply_suppressions(
    findings: Vec<Finding>,
    config: &Config,
    inline_suppressions: &HashMap<(PathBuf, usize), Vec<String>>,
) -> Vec<Finding> {
    findings
        .into_iter()
        .filter(|f| {
            // Check detector enabled
            if !config.is_detector_enabled(&f.detector_name) {
                return false;
            }

            // Check file exclusion
            for loc in &f.locations {
                if config.is_file_excluded(&loc.file) {
                    return false;
                }
            }

            // Check inline suppression
            for loc in &f.locations {
                let key = (loc.file.clone(), loc.start_line);
                if let Some(suppressed) = inline_suppressions.get(&key) {
                    if suppressed.iter().any(|s| s == "*" || *s == f.detector_name) {
                        return false;
                    }
                }
            }

            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::finding::{Confidence, SourceLocation};

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.global.severity_threshold, "low");
        assert!(config.is_detector_enabled("any-detector"));
    }

    #[test]
    fn test_parse_config() {
        let toml = r#"
[global]
severity_threshold = "medium"

[detectors.unsafe-unwrap]
enabled = false

[suppressions]
files = ["tests/**"]
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.severity_threshold(), Severity::Medium);
        assert!(!config.is_detector_enabled("unsafe-unwrap"));
        assert!(config.is_detector_enabled("missing-addr-validate"));
        assert!(config.is_file_excluded(Path::new("tests/test_contract.rs")));
        assert!(!config.is_file_excluded(Path::new("src/contract.rs")));
    }

    #[test]
    fn test_inline_suppression_parsing() {
        let mut source_map = HashMap::new();
        source_map.insert(
            PathBuf::from("test.rs"),
            "// cosmwasm-guard-ignore: unsafe-unwrap\nlet x = foo.unwrap();\n// cosmwasm-guard-ignore\nlet y = bar.unwrap();\n".to_string(),
        );

        let suppressions = parse_inline_suppressions(&source_map);
        // Line 2 (1-based) should be suppressed for unsafe-unwrap
        let key = (PathBuf::from("test.rs"), 2);
        assert!(suppressions.contains_key(&key));
        assert_eq!(suppressions[&key], vec!["unsafe-unwrap"]);

        // Line 4 should be suppressed for all (wildcard)
        let key = (PathBuf::from("test.rs"), 4);
        assert!(suppressions.contains_key(&key));
        assert_eq!(suppressions[&key], vec!["*"]);
    }

    #[test]
    fn test_apply_suppressions() {
        let config = Config::default();
        let mut inline = HashMap::new();
        inline.insert(
            (PathBuf::from("test.rs"), 5),
            vec!["unsafe-unwrap".to_string()],
        );

        let findings = vec![
            Finding {
                detector_name: "unsafe-unwrap".to_string(),
                title: "test".to_string(),
                description: "test".to_string(),
                severity: Severity::Medium,
                confidence: Confidence::High,
                locations: vec![SourceLocation {
                    file: PathBuf::from("test.rs"),
                    start_line: 5,
                    end_line: 5,
                    start_col: 0,
                    end_col: 0,
                    snippet: None,
                }],
                recommendation: None,
                fix: None,
            },
            Finding {
                detector_name: "missing-addr-validate".to_string(),
                title: "test2".to_string(),
                description: "test2".to_string(),
                severity: Severity::Medium,
                confidence: Confidence::Medium,
                locations: vec![SourceLocation {
                    file: PathBuf::from("test.rs"),
                    start_line: 10,
                    end_line: 10,
                    start_col: 0,
                    end_col: 0,
                    snippet: None,
                }],
                recommendation: None,
                fix: None,
            },
        ];

        let filtered = apply_suppressions(findings, &config, &inline);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].detector_name, "missing-addr-validate");
    }
}
