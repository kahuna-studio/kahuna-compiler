pub mod port_manifest;
pub mod midi;
pub mod patch;
pub mod io_nodes;

use std::any::TypeId;

// ============================================================================
// Core Reflection & Metadata Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDir {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortKind {
    Signal,    // Continuous audio/control stream (~)
    Parameter, // Stream carrying parameter updates (@)
    Event,     // Discrete asynchronous events (^)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rate {
    AudioRate,
    ControlRate,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Range {
    Float(f32, f32),
    Int(i64, i64),
}

#[derive(Clone, Copy)]
pub struct PortType {
    pub type_id: fn() -> TypeId,
    pub canonical_path: fn() -> &'static str,
    pub enum_variants: Option<&'static [&'static str]>,
}

impl PortType {
    pub const fn of<T: 'static>() -> Self {
        Self {
            type_id: TypeId::of::<T>,
            canonical_path: std::any::type_name::<T>,
            enum_variants: None,
        }
    }

    pub const fn of_enum<T: 'static>(variants: &'static [&'static str]) -> Self {
        Self {
            type_id: TypeId::of::<T>,
            canonical_path: std::any::type_name::<T>,
            enum_variants: Some(variants),
        }
    }

    #[inline]
    pub fn matches(&self, other: &PortType) -> bool {
        (self.type_id)() == (other.type_id)()
    }

    pub fn short_name(&self) -> &'static str {
        (self.canonical_path)()
            .rsplit("::")
            .next()
            .unwrap_or((self.canonical_path)())
    }
}

impl std::fmt::Debug for PortType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortType")
            .field("canonical_path", &self.canonical_path)
            .field("enum_variants", &self.enum_variants)
            .finish()
    }
}

impl PartialEq for PortType {
    fn eq(&self, other: &Self) -> bool {
        self.matches(other)
    }
}

/// Static description of a port for the visual patch editor.
///
/// `required`:
/// - Parameter/Signal inputs: derived from `Option<T>` vs `T` in module source.
/// - Event inputs: always `Some(false)` — unconnected is a valid empty stream.
/// - Outputs: always `None` — the concept doesn't apply to outputs.
#[derive(Debug, Clone, PartialEq)]
pub struct PortSpec {
    pub name: &'static str,
    pub direction: PortDir,
    pub stream_type: PortKind,
    pub rate: Option<Rate>,
    pub range: Option<Range>,
    pub required: Option<bool>,
    pub value_type: PortType,
}

// ============================================================================
// Module Lifecycle & Environment
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub struct ModuleEnv {
    pub sample_rate: u32,
    pub max_buffer_size: usize,
}

pub trait KahunaModule: Sized {
    const PORTS: &'static [PortSpec];
    fn new(env: &ModuleEnv) -> Self;
}
