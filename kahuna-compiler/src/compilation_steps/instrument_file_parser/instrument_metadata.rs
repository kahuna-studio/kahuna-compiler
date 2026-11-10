use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use eyre::{Result, eyre};

/// Parsed instrument.toml metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstrumentMetadata {
    /// Name of the instrument (used as crate name)
    pub name: String,
    /// Vendor name
    pub vendor: String,
    /// Version string (semver)
    pub version: String,
    /// Unit libraries this instrument uses, written like Cargo dependencies
    /// (`kahuna-std = "0.1"`, `my-units = { path = "../my-units" }`, ...).
    /// The instrument's own library in `units/` is found automatically and not listed here.
    #[serde(default)]
    pub libraries: BTreeMap<String, toml::Value>,
}

/// How patches refer to the instrument's own library in `units/`, e.g. `self/sine-osc`.
pub const SELF_LIBRARY: &str = "self";

impl InstrumentMetadata {
    /// Parse instrument.toml from a file path
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .map_err(|e| eyre!("Failed to read instrument.toml: {}", e))?;

        Self::from_str(&contents)
    }

    /// Parse instrument.toml from a string
    pub fn from_str(contents: &str) -> Result<Self> {
        let metadata: InstrumentMetadata = toml::from_str(contents)
            .map_err(|e| eyre!("Failed to parse instrument.toml: {}", e))?;

        // Validate
        metadata.validate()?;
        Ok(metadata)
    }

    /// Validate the metadata
    fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(eyre!("Instrument name cannot be empty"));
        }

        // Validate crate name rules (alphanumeric + hyphen/underscore, start with letter/underscore)
        if !self.name.chars().next().map_or(false, |c| c.is_alphabetic() || c == '_') {
            return Err(eyre!(
                "Instrument name must start with a letter or underscore, got: {}",
                self.name
            ));
        }

        if !self
            .name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(eyre!(
                "Instrument name must only contain alphanumeric characters, hyphens, and underscores"
            ));
        }

        if self.vendor.is_empty() {
            return Err(eyre!("Vendor name cannot be empty"));
        }

        if self.version.is_empty() {
            return Err(eyre!("Version cannot be empty"));
        }

        if self.libraries.contains_key(SELF_LIBRARY) {
            return Err(eyre!(
                "'{SELF_LIBRARY}' is reserved for the instrument's own library in units/ and cannot be listed in [libraries]"
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_metadata() {
        let toml_str = r#"
name = "my-synth"
vendor = "My Company"
version = "0.1.0"
"#;
        let metadata = InstrumentMetadata::from_str(toml_str).unwrap();
        assert_eq!(metadata.name, "my-synth");
        assert_eq!(metadata.vendor, "My Company");
        assert_eq!(metadata.version, "0.1.0");
        assert!(metadata.libraries.is_empty(), "[libraries] is optional");
    }

    #[test]
    fn test_parse_with_libraries() {
        let toml_str = r#"
name = "my-synth"
vendor = "My Company"
version = "0.1.0"

[libraries]
kahuna-std = "0.1"
my-effects = { path = "../my-effects" }
"#;
        let metadata = InstrumentMetadata::from_str(toml_str).unwrap();
        assert_eq!(metadata.libraries.len(), 2);
        assert_eq!(metadata.libraries["kahuna-std"].as_str(), Some("0.1"));
        assert_eq!(metadata.libraries["my-effects"]["path"].as_str(), Some("../my-effects"));
    }

    #[test]
    fn test_self_is_reserved() {
        let toml_str = r#"
name = "my-synth"
vendor = "My Company"
version = "0.1.0"

[libraries]
self = { path = "units" }
"#;
        assert!(InstrumentMetadata::from_str(toml_str).is_err());
    }

    #[test]
    fn test_validate_empty_name() {
        let toml_str = r#"
name = ""
vendor = "My Company"
version = "0.1.0"
"#;
        let result = InstrumentMetadata::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_invalid_name() {
        let toml_str = r#"
name = "9invalid"
vendor = "My Company"
version = "0.1.0"
"#;
        let result = InstrumentMetadata::from_str(toml_str);
        assert!(result.is_err());
    }
}