use anyhow::Result;
use cosmwasm_guard::finding::Severity;
use cosmwasm_guard::report::AnalysisReport;
use serde_json::json;

/// Print SARIF 2.1.0 output for GitHub Code Scanning integration
pub fn print(report: &AnalysisReport) -> Result<()> {
    // Build stable rule descriptions from detector metadata (not per-finding titles)
    let all_dets = cosmwasm_guard_detectors::all_detectors();
    let rules: Vec<serde_json::Value> = report
        .findings
        .iter()
        .map(|f| &f.detector_name)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .map(|name| {
            let det = all_dets.iter().find(|d| d.name() == name);
            let finding = report.findings.iter().find(|f| &f.detector_name == name);
            json!({
                "id": name,
                "shortDescription": {
                    "text": det.map_or_else(
                        || finding.map_or("", |f| &f.detector_name).to_string(),
                        |d| d.description().to_string()
                    )
                },
                "defaultConfiguration": {
                    "level": finding.map_or("warning", |f| severity_to_sarif_level(&f.severity))
                }
            })
        })
        .collect();

    let results: Vec<serde_json::Value> = report
        .findings
        .iter()
        .map(|f| {
            let locations: Vec<serde_json::Value> = f
                .locations
                .iter()
                .map(|loc| {
                    json!({
                        "physicalLocation": {
                            "artifactLocation": {
                                "uri": loc.file.display().to_string()
                            },
                            "region": {
                                "startLine": loc.start_line,
                                "startColumn": loc.start_col + 1,
                                "endLine": loc.end_line,
                                "endColumn": loc.end_col + 1
                            }
                        }
                    })
                })
                .collect();

            let mut result = json!({
                "ruleId": f.detector_name,
                "level": severity_to_sarif_level(&f.severity),
                "message": {
                    "text": f.description
                },
                "locations": locations
            });

            // Add fix suggestions if present
            if let Some(fix) = &f.fix {
                result["fixes"] = json!([{
                    "description": {
                        "text": fix.description
                    },
                    "artifactChanges": [{
                        "artifactLocation": {
                            "uri": fix.location.file.display().to_string()
                        },
                        "replacements": [{
                            "deletedRegion": {
                                "startLine": fix.location.start_line,
                                "startColumn": fix.location.start_col + 1,
                                "endLine": fix.location.end_line,
                                "endColumn": fix.location.end_col + 1
                            },
                            "insertedContent": {
                                "text": fix.replacement_text
                            }
                        }]
                    }]
                }]);
            }

            result
        })
        .collect();

    let sarif = json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "cosmwasm-guard",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": "https://github.com/safestackai/cosmwasm-guard",
                    "rules": rules
                }
            },
            "results": results
        }]
    });

    let json = serde_json::to_string_pretty(&sarif)?;
    println!("{json}");
    Ok(())
}

fn severity_to_sarif_level(severity: &Severity) -> &'static str {
    match severity {
        Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low => "note",
        Severity::Informational => "note",
    }
}
