use std::path::Path;

use anyhow::Result;
use cosmwasm_guard::config::Config;

pub fn run() -> Result<()> {
    let path = Path::new(".cosmwasm-guard.toml");
    if path.exists() {
        eprintln!("Config file already exists: {}", path.display());
        return Ok(());
    }
    std::fs::write(path, Config::default_toml())?;
    println!("Created {}", path.display());
    Ok(())
}
