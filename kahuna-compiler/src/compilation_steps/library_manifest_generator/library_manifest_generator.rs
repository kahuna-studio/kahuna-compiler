//! Builds a unit library's manifest by reading its source with `syn`. Nothing is compiled.
//!
//! Starting at the library root, the scanner follows `mod` declarations the same way rustc
//! does and collects every struct marked `#[kahuna_module]`.

use std::fs;
use std::path::{Path, PathBuf};

use eyre::{bail, eyre, Result, WrapErr};
use kahuna_unit_syntax::{parse_module, Direction, LiteralValue, PortConfiguration, UnitAttributes};
use quote::ToTokens;
use syn::ext::IdentExt;
use syn::{Attribute, DeriveInput, Item, ItemStruct, Meta, Type, Visibility};

use crate::compilation_steps::library_manifest_generator::library_manifest::{
  PortDirection, PortEntry, Rate, StreamType, UnitEntry, UnitKind, LibraryManifest, FORMAT_VERSION, LIBRARY_MANIFEST_FILE,
};

const MODULE_MARKER: &str = "kahuna_module";

/// Scans the library in `crate_dir` and writes its manifest next to its Cargo.toml.
pub fn write_library_manifest(crate_dir: &Path) -> Result<PathBuf> {
  let manifest = generate_library_manifest(crate_dir)?;
  let path = crate_dir.join(LIBRARY_MANIFEST_FILE);
  fs::write(&path, to_json(&manifest)?).wrap_err_with(|| format!("Cannot write {}", path.display()))?;
  Ok(path)
}

pub fn to_json(manifest: &LibraryManifest) -> Result<String> {
  Ok(serde_json::to_string_pretty(manifest)? + "\n")
}

pub fn generate_library_manifest(crate_dir: &Path) -> Result<LibraryManifest> {
  let library_root = library_root(crate_dir)?;
  let children_dir = library_root
    .parent()
    .expect("library root is a file inside the crate")
    .to_path_buf();

  let mut scanner = Scanner { units: Vec::new() };
  scanner.scan_file(&library_root, &children_dir, &[], true)?;
  check_unique_ids(&scanner.units)?;

  Ok(LibraryManifest {
    format_version: FORMAT_VERSION,
    units: scanner.units,
  })
}

/// `[lib] path` from Cargo.toml, or Cargo's default `src/lib.rs`.
fn library_root(crate_dir: &Path) -> Result<PathBuf> {
  let cargo_toml_path = crate_dir.join("Cargo.toml");
  let cargo_toml: toml::Table = fs::read_to_string(&cargo_toml_path)
    .wrap_err_with(|| format!("Cannot read {}", cargo_toml_path.display()))?
    .parse()
    .wrap_err_with(|| format!("Cannot parse {}", cargo_toml_path.display()))?;

  let configured_path = cargo_toml
    .get("lib")
    .and_then(|lib| lib.get("path"))
    .and_then(|path| path.as_str());
  let library_root = crate_dir.join(configured_path.unwrap_or("src/lib.rs"));

  if !library_root.is_file() {
    bail!(
      "{} is not a library crate: {} not found. Unit libraries must be Rust library crates.",
      crate_dir.display(),
      library_root.display()
    );
  }
  Ok(library_root)
}

struct Scanner {
  units: Vec<UnitEntry>,
}

impl Scanner {
  /// `children_dir` is where the files of modules declared in `file` live.
  /// `public` tells whether everything up to here is reachable from outside the crate.
  fn scan_file(&mut self, file: &Path, children_dir: &Path, module_path: &[String], public: bool) -> Result<()> {
    let source = fs::read_to_string(file).wrap_err_with(|| format!("Cannot read {}", file.display()))?;
    let syntax = syn::parse_file(&source).map_err(|error| located(file, error))?;
    self.scan_items(&syntax.items, file, children_dir, module_path, public)
  }

  fn scan_items(
    &mut self,
    items: &[Item],
    file: &Path,
    children_dir: &Path,
    module_path: &[String],
    public: bool,
  ) -> Result<()> {
    for item in items {
      match item {
        Item::Mod(module) if !is_test_only(&module.attrs) => {
          let name = module.ident.unraw().to_string();
          let child_path = [module_path, &[name.clone()]].concat();
          let child_public = public && is_public(&module.vis);
          let grandchildren_dir = children_dir.join(&name);

          match &module.content {
            Some((_, items)) => self.scan_items(items, file, &grandchildren_dir, &child_path, child_public)?,
            None => {
              let module_file = module_file(module, file, children_dir, &name)?;
              self.scan_file(&module_file, &grandchildren_dir, &child_path, child_public)?;
            }
          }
        }
        Item::Struct(item_struct) => {
          if let Some(marker) = find_marker(&item_struct.attrs) {
            let unit = read_module(item_struct, marker, file, module_path, public)?;
            self.units.push(unit);
          }
        }
        _ => {}
      }
    }
    Ok(())
  }
}

/// Resolves `mod name;` to `name.rs` or `name/mod.rs`, as rustc does.
fn module_file(module: &syn::ItemMod, file: &Path, children_dir: &Path, name: &str) -> Result<PathBuf> {
  if module.attrs.iter().any(|attribute| attribute.path().is_ident("path")) {
    bail!("{}: `#[path]` on modules is not supported (module `{name}`)", file.display());
  }

  let candidates = [children_dir.join(format!("{name}.rs")), children_dir.join(name).join("mod.rs")];
  candidates
    .iter()
    .find(|candidate| candidate.is_file())
    .cloned()
    .ok_or_else(|| {
      eyre!(
        "{}: file for module `{name}` not found; looked for {} and {}",
        file.display(),
        candidates[0].display(),
        candidates[1].display()
      )
    })
}

fn read_module(
  item_struct: &ItemStruct,
  marker: &Attribute,
  file: &Path,
  module_path: &[String],
  public: bool,
) -> Result<UnitEntry> {
  let rust_path = [module_path, &[item_struct.ident.unraw().to_string()]].concat().join("::");

  if !(public && is_public(&item_struct.vis)) {
    bail!(
      "{}: unit `{rust_path}` is not reachable from outside the crate; make it and all modules on its path `pub`",
      file.display()
    );
  }

  let attributes = match &marker.meta {
    Meta::Path(_) => UnitAttributes::default(),
    Meta::List(list) => UnitAttributes::parse(list.tokens.clone()).map_err(|error| located(file, error))?,
    Meta::NameValue(_) => bail!("{}: expected `#[{MODULE_MARKER}]` or `#[{MODULE_MARKER}(...)]`", file.display()),
  };

  let input = DeriveInput::from(item_struct.clone());
  let module = parse_module(&input, &attributes).map_err(|error| located(file, error))?;

  Ok(UnitEntry {
    id: module.id,
    kind: UnitKind::Module,
    rust_path,
    ports: module.ports.iter().map(port_entry).collect(),
  })
}

fn port_entry(port: &PortConfiguration) -> PortEntry {
  PortEntry {
    direction: match port.direction {
      Direction::Input => PortDirection::Input,
      Direction::Output => PortDirection::Output,
    },
    name: port.identifier.unraw().to_string(),
    stream_type: match port.stream_kind {
      kahuna_core::StreamKind::Signal => StreamType::Signal,
      kahuna_core::StreamKind::Parameter => StreamType::Parameter,
      kahuna_core::StreamKind::Event => StreamType::Event,
    },
    rate: port.rate().map(|rate| match rate {
      kahuna_core::Rate::AudioRate => Rate::Audio,
      kahuna_core::Rate::ControlRate => Rate::Control,
    }),
    value_type: value_type_name(&port.value_type),
    required: port.required(),
    default: port.default_value.as_ref().map(|default| match default.literal {
      LiteralValue::Bool(value) => serde_json::Value::from(value),
      LiteralValue::Integer(value) => serde_json::Value::from(value),
      LiteralValue::Float(value) => serde_json::Value::from(value),
    }),
  }
}

/// The last path segment (`kahuna_core::Unipolar` becomes `Unipolar`), or the type as written.
fn value_type_name(value_type: &Type) -> String {
  match value_type {
    Type::Path(type_path) if type_path.qself.is_none() => type_path
      .path
      .segments
      .last()
      .map(|segment| segment.to_token_stream().to_string().replace(' ', ""))
      .unwrap_or_default(),
    _ => value_type.to_token_stream().to_string().replace(' ', ""),
  }
}

fn check_unique_ids(units: &[UnitEntry]) -> Result<()> {
  for (index, unit) in units.iter().enumerate() {
    if let Some(duplicate) = units[index + 1..].iter().find(|other| other.id == unit.id) {
      bail!(
        "units `{}` and `{}` both have the id `{}`; set a different one with #[{MODULE_MARKER}(id = \"...\")]",
        unit.rust_path,
        duplicate.rust_path,
        unit.id
      );
    }
  }
  Ok(())
}

fn find_marker(attributes: &[Attribute]) -> Option<&Attribute> {
  attributes.iter().find(|attribute| {
    attribute
      .path()
      .segments
      .last()
      .is_some_and(|segment| segment.ident == MODULE_MARKER)
  })
}

fn is_test_only(attributes: &[Attribute]) -> bool {
  attributes.iter().any(|attribute| {
    attribute.path().is_ident("cfg")
      && matches!(&attribute.meta, Meta::List(list) if list.tokens.to_string() == "test")
  })
}

fn is_public(visibility: &Visibility) -> bool {
  matches!(visibility, Visibility::Public(_))
}

fn located(file: &Path, error: syn::Error) -> eyre::Report {
  let start = error.span().start();
  eyre!("{}:{}:{}: {error}", file.display(), start.line, start.column + 1)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/libraries").join(name)
  }

  /// Writes `files` into a fresh crate directory below the system temp dir.
  fn temporary_crate(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kahuna-manifest-generator-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for (path, contents) in [("Cargo.toml", "[package]\nname = \"units\"\n")].iter().chain(files) {
      let path = dir.join(path);
      fs::create_dir_all(path.parent().unwrap()).unwrap();
      fs::write(path, contents).unwrap();
    }
    dir
  }

  #[test]
  fn matches_the_committed_manifest_of_the_fixture_crate() {
    let crate_dir = fixture("envelopes");
    let expected = fs::read_to_string(crate_dir.join(LIBRARY_MANIFEST_FILE)).unwrap();

    let manifest = generate_library_manifest(&crate_dir).unwrap();

    assert_eq!(to_json(&manifest).unwrap(), expected);
  }

  #[test]
  fn rejects_units_in_private_modules() {
    let crate_dir = temporary_crate(
      "private",
      &[
        ("src/lib.rs", "mod hidden;"),
        ("src/hidden.rs", "#[kahuna_module] pub struct Osc { #[output] out: Audio }"),
      ],
    );

    let error = generate_library_manifest(&crate_dir).unwrap_err();

    assert!(format!("{error}").contains("`hidden::Osc` is not reachable"), "{error}");
  }

  #[test]
  fn rejects_duplicate_ids() {
    let crate_dir = temporary_crate(
      "duplicates",
      &[(
        "src/lib.rs",
        "#[kahuna_module] pub struct Osc { #[output] out: Audio }
         #[kahuna_module(id = \"osc\")] pub struct Other { #[output] out: Audio }",
      )],
    );

    let error = generate_library_manifest(&crate_dir).unwrap_err();

    assert!(format!("{error}").contains("both have the id `osc`"), "{error}");
  }

  #[test]
  fn reports_the_location_of_invalid_ports() {
    let crate_dir = temporary_crate(
      "invalid",
      &[("src/lib.rs", "#[kahuna_module]\npub struct Osc {\n  #[input(default = A)] a: Parameter<f32>,\n}")],
    );

    let error = generate_library_manifest(&crate_dir).unwrap_err();

    assert!(format!("{error}").contains("lib.rs:3:"), "{error}");
  }

  #[test]
  fn rejects_crates_without_a_library() {
    let crate_dir = temporary_crate("binary", &[("src/main.rs", "fn main() {}")]);

    let error = generate_library_manifest(&crate_dir).unwrap_err();

    assert!(format!("{error}").contains("is not a library crate"), "{error}");
  }
}
