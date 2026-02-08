use std::fmt;

use super::types::{Finding, SourceLocation};

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file.display(), self.start_line)
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[{}] {} ({})",
            self.severity, self.title, self.detector_name
        )?;
        if !self.locations.is_empty() {
            write!(f, " at {}", self.locations[0])?;
        }
        Ok(())
    }
}
