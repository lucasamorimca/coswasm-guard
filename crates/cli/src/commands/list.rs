use anyhow::Result;

pub fn run() -> Result<()> {
    let detectors = cosmwasm_guard_detectors::all_detectors();

    println!(
        "{:<30} {:<10} {:<12} Description",
        "Name", "Severity", "Confidence"
    );
    println!("{}", "-".repeat(90));

    for d in &detectors {
        println!(
            "{:<30} {:<10} {:<12} {}",
            d.name(),
            d.severity(),
            d.confidence(),
            d.description()
        );
    }

    println!("\nTotal: {} detectors", detectors.len());
    Ok(())
}
