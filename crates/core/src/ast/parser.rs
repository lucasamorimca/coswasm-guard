use std::path::Path;

use anyhow::{Context, Result};

/// Parse a Rust source file into a syn AST
pub fn parse_file(path: &Path) -> Result<syn::File> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    syn::parse_file(&content).with_context(|| format!("Failed to parse file: {}", path.display()))
}

/// Parse Rust source code from a string (useful for testing)
pub fn parse_source(source: &str) -> Result<syn::File> {
    syn::parse_file(source).map_err(|e| anyhow::anyhow!("Parse error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_rust() {
        let source = "fn main() {}";
        let result = parse_source(source);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_invalid_rust() {
        let source = "fn main( {}";
        let result = parse_source(source);
        assert!(result.is_err());
    }
}
