use anyhow::Result;
use cosmwasm_guard::report::AnalysisReport;

pub fn print(report: &AnalysisReport) -> Result<()> {
    let json = serde_json::to_string_pretty(report)?;
    println!("{json}");
    Ok(())
}
