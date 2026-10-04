use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use eyre::{Result, eyre};

/// Represents a build target for the instrument (VST3, CLAP, standalone, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum BuildTarget {
    Vst3,
    Clap,
    Standalone,
}

/// Parsed instrument.toml metadata
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstrumentMetadata {
    /// Name of the instrument (used as crate name)
    pub name: String,
    /// Vendor name
    pub vendor: String,
    /// Version string (semver)
    pub version: String,
    /// Build targets this instrument supports
    pub targets: Vec<BuildTarget>,
    /// Optional: external unit crates to use (beyond kahuna-std and units/)
    #[serde(default)]
    pub unit_crates: BTreeMap<String, String>,
}

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

        if self.targets.is_empty() {
            return Err(eyre!("At least one build target must be specified"));
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
targets = ["vst3", "standalone"]
"#;
        let metadata = InstrumentMetadata::from_str(toml_str).unwrap();
        assert_eq!(metadata.name, "my-synth");
        assert_eq!(metadata.vendor, "My Company");
        assert_eq!(metadata.version, "0.1.0");
        assert_eq!(metadata.targets.len(), 2);
    }

    #[test]
    fn test_parse_with_unit_crates() {
        let toml_str = r#"
name = "my-synth"
vendor = "My Company"
version = "0.1.0"
targets = ["vst3"]

[unit_crates]
my_effects = "../my_effects"
"#;
        let metadata = InstrumentMetadata::from_str(toml_str).unwrap();
        assert_eq!(metadata.unit_crates.len(), 1);
        assert_eq!(metadata.unit_crates.get("my_effects"), Some(&"../my_effects".to_string()));
    }

    #[test]
    fn test_validate_empty_name() {
        let toml_str = r#"
name = ""
vendor = "My Company"
version = "0.1.0"
targets = ["vst3"]
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
targets = ["vst3"]
"#;
        let result = InstrumentMetadata::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_empty_targets() {
        let toml_str = r#"
name = "my-synth"
vendor = "My Company"
version = "0.1.0"
targets = []
"#;
        let result = InstrumentMetadata::from_str(toml_str);
        assert!(result.is_err());
    }
}
