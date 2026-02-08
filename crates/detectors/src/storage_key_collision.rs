use std::collections::HashMap;

use cosmwasm_guard::detector::{AnalysisContext, Detector};
use cosmwasm_guard::finding::*;

/// Detects duplicate storage key strings across state declarations.
/// Two state items sharing the same key will corrupt each other's data.
pub struct StorageKeyCollision;

impl Detector for StorageKeyCollision {
    fn name(&self) -> &str {
        "storage-key-collision"
    }

    fn description(&self) -> &str {
        "Detects duplicate storage key strings across state declarations"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn confidence(&self) -> Confidence {
        Confidence::High
    }

    fn detect(&self, ctx: &AnalysisContext) -> Vec<Finding> {
        let mut findings = Vec::new();
        let mut seen: HashMap<&str, &str> = HashMap::new(); // key -> first item name

        for item in &ctx.contract.state_items {
            let Some(key) = &item.storage_key else {
                continue;
            };
            if let Some(&first_name) = seen.get(key.as_str()) {
                findings.push(Finding {
                    detector_name: self.name().to_string(),
                    title: format!(
                        "Storage key collision: `{}` and `{}` share key \"{}\"",
                        first_name, item.name, key
                    ),
                    description: format!(
                        "State items `{}` and `{}` both use storage key \"{}\". \
                         This causes data corruption — writes to one will overwrite the other.",
                        first_name, item.name, key
                    ),
                    severity: Severity::High,
                    confidence: Confidence::High,
                    locations: vec![SourceLocation {
                        file: item.span.file.clone(),
                        start_line: item.span.start_line,
                        end_line: item.span.end_line,
                        start_col: item.span.start_col,
                        end_col: item.span.end_col,
                        snippet: None,
                    }],
                    recommendation: Some(
                        "Use unique storage key strings for each state item.".to_string(),
                    ),
                });
            } else {
                seen.insert(key, &item.name);
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_guard::ast::{parse_source, ContractVisitor};
    use cosmwasm_guard::ir::builder::IrBuilder;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn analyze(source: &str) -> Vec<Finding> {
        let ast = parse_source(source).unwrap();
        let contract = ContractVisitor::extract(PathBuf::from("test.rs"), ast);
        let ir = IrBuilder::build_contract(&contract);
        let mut sources = HashMap::new();
        sources.insert(PathBuf::from("test.rs"), source.to_string());
        let ctx = AnalysisContext::new(&contract, &ir, &sources);
        StorageKeyCollision.detect(&ctx)
    }

    #[test]
    fn test_detects_duplicate_keys() {
        let source = r#"
            const CONFIG: Item<Config> = Item::new("config");
            const SETTINGS: Item<Settings> = Item::new("config");
        "#;
        let findings = analyze(source);
        assert!(!findings.is_empty());
        assert_eq!(findings[0].detector_name, "storage-key-collision");
    }

    #[test]
    fn test_no_finding_unique_keys() {
        let source = r#"
            const CONFIG: Item<Config> = Item::new("config");
            const BALANCES: Map<&str, Uint128> = Map::new("balances");
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_no_finding_no_state() {
        let source = r#"
            fn helper() -> u32 { 42 }
        "#;
        let findings = analyze(source);
        assert!(findings.is_empty());
    }
}
