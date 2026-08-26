use std::marker::PhantomData;

pub mod port_manifest;
pub mod midi;
pub mod patch;
pub mod io_nodes;

use std::any::TypeId;
use crate::midi::MidiEvent;
// ============================================================================
// Core Reflection & Metadata Types
// ============================================================================


pub struct Signal<T>(PhantomData<T>);
pub struct Parameter<T>(PhantomData<T>);
pub struct Event<T>(PhantomData<T>);

pub type Unipolar = f32;
pub type Bipolar = f32;

pub type Audio = Signal<Bipolar>;
pub type Control = Parameter<Unipolar>;
pub type Midi = Event<MidiEvent>;

pub trait StreamType {
    type Value: 'static;
    type Input<'a>;
    type Output<'a>;
    const KIND: StreamKind;
    const DEFAULT_RATE: Option<Rate>;
}

impl<T: 'static> StreamType for Signal<T> {
    type Value = T;
    type Input<'a> = &'a [T];
    type Output<'a> = &'a mut [T];
    const KIND: StreamKind = StreamKind::Signal;
    const DEFAULT_RATE: Option<Rate> = Some(Rate::AudioRate);
}

impl<T: 'static> StreamType for Parameter<T> {
    type Value = T;
    type Input<'a> = T;
    type Output<'a> = &'a mut T;
    const KIND: StreamKind = StreamKind::Parameter;
    const DEFAULT_RATE: Option<Rate> = Some(Rate::ControlRate);
}

impl<T: 'static> StreamType for Event<T> {
    type Value = T;
    type Input<'a> = &'a [T];
    type Output<'a> = &'a mut Vec<T>;
    const KIND: StreamKind = StreamKind::Event;
    const DEFAULT_RATE: Option<Rate> = None;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamKind {
    Signal,    // Continuous audio/control stream (~)
    Parameter, // Stream carrying parameter updates (@)
    Event,     // Discrete asynchronous events (^)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rate {
    AudioRate,
    ControlRate,
}

#[derive(Clone, Copy)]
pub struct ValueType {
    pub type_id: fn() -> TypeId,
    pub canonical_path: fn() -> &'static str,
    pub enum_variants: Option<&'static [&'static str]>,
}

impl ValueType {
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
    pub fn matches(&self, other: &ValueType) -> bool {
        (self.type_id)() == (other.type_id)()
    }

    pub fn short_name(&self) -> &'static str {
        (self.canonical_path)()
            .rsplit("::")
            .next()
            .unwrap_or((self.canonical_path)())
    }
}

impl std::fmt::Debug for ValueType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortType")
            .field("canonical_path", &self.canonical_path)
            .field("enum_variants", &self.enum_variants)
            .finish()
    }
}

impl PartialEq for ValueType {
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
pub struct PortSpecification {
    pub name: &'static str,
    pub direction: PortDirection,
    pub stream_kind: StreamKind,
    pub rate: Option<Rate>,
    pub required: Option<bool>,
    pub value_type: ValueType,
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
    const PORTS: &'static [PortSpecification];
    fn new(env: &ModuleEnv) -> Self;
}
