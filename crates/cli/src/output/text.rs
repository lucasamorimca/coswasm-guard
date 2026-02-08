use anyhow::Result;
use colored::Colorize;
use cosmwasm_guard::finding::Severity;
use cosmwasm_guard::report::AnalysisReport;

pub fn print(report: &AnalysisReport, quiet: bool, no_color: bool) -> Result<()> {
    if no_color {
        colored::control::set_override(false);
    }

    if !quiet {
        println!();
        println!("{}", "  cosmwasm-guard - CosmWasm Static Analysis".bold());
        println!("  Files analyzed: {}", report.files_analyzed.len());
        println!();
    }

    if report.findings.is_empty() {
        if !quiet {
            println!("  {} No issues found.", "✓".green().bold());
            println!();
        }
        return Ok(());
    }

    for finding in &report.findings {
        let severity_label = match finding.severity {
            Severity::High => "HIGH".red().bold(),
            Severity::Medium => "MEDIUM".yellow().bold(),
            Severity::Low => "LOW".blue(),
            Severity::Informational => "INFO".dimmed(),
        };

        println!(
            "  [{}] {} ({})",
            severity_label, finding.title, finding.detector_name
        );
        println!("    {}", finding.description);

        for loc in &finding.locations {
            println!(
                "    {} {}:{}",
                "-->".dimmed(),
                loc.file.display(),
                loc.start_line
            );
            if let Some(snippet) = &loc.snippet {
                for line in snippet.lines() {
                    println!("    {} {}", "|".dimmed(), line);
                }
            }
        }

        if let Some(rec) = &finding.recommendation {
            println!("    {} {}", "Fix:".green(), rec);
        }
        println!();
    }

    if !quiet {
        println!("{}", "  Summary".bold().underline());
        println!("    High:          {}", report.findings_by_severity.high);
        println!("    Medium:        {}", report.findings_by_severity.medium);
        println!("    Low:           {}", report.findings_by_severity.low);
        println!(
            "    Informational: {}",
            report.findings_by_severity.informational
        );
        println!("    Total:         {}", report.total_findings);
        println!();
    }

    Ok(())
}
