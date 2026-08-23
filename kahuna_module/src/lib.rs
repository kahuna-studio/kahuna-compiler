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
    Event,     // Discrete asynchronous events ($)
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

/// Strongly-typed port representation with full namespace awareness.
#[derive(Clone, Copy)]
pub struct PortType {
    /// Compiler TypeId getter for fast, unambiguous wire-compatibility checks.
    pub type_id: fn() -> TypeId,

    /// Full canonical namespace path: e.g. "kahuna_synth::modules::sine_osc::Waveform"
    pub canonical_path: fn() -> &'static str,

    /// Optional enum variant names (for the visual patch editor dropdowns)
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
#[derive(Debug, Clone, PartialEq)]
pub struct PortSpec {
    pub name: &'static str,
    pub dir: PortDir,
    pub kind: PortKind,
    pub rate: Option<Rate>,
    pub range: Option<Range>,
    pub port_type: PortType,
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
    /// Static metadata table introspected by the visual patch editor.
    const PORTS: &'static [PortSpec];

    /// Construct the module (with internal DSP state initialized).
    fn new(env: &ModuleEnv) -> Self;
}

// ============================================================================
// Macro for Kahuna Modules
// ============================================================================

#[macro_export]
macro_rules! kahuna_module {
    (
        $(#[$meta:meta])*
        struct $name:ident {
            $( pub $state_field:ident : $state_type:ty ),* $(,)?
        }

        inputs {
            $( $in_sig:tt $in_name:ident : $in_type:ty $( [ $( $in_key:ident = $in_val:expr ),* $(,)? ] )? ),* $(,)?
        }

        outputs {
            $( $out_sig:tt $out_name:ident : $out_type:ty $( [ $( $out_key:ident = $out_val:expr ),* $(,)? ] )? ),* $(,)?
        }
    ) => {
        $(#[$meta])*
        pub struct $name {
            $( pub $state_field : $state_type, )*
        }

        #[allow(non_snake_case)]
        pub struct Inputs<'a> {
            $(
                pub $in_name : $crate::__in_port_type!($in_sig, $in_type, 'a),
            )*
            pub _phantom: ::std::marker::PhantomData<&'a ()>,
        }

        #[allow(non_snake_case)]
        pub struct Outputs<'a> {
            $(
                pub $out_name : $crate::__out_port_type!($out_sig, $out_type, 'a),
            )*
            pub _phantom: ::std::marker::PhantomData<&'a ()>,
        }

        impl $crate::KahunaModule for $name {
            const PORTS: &'static [$crate::PortSpec] = &[
                $(
                    $crate::__make_in_port_spec!(
                        $in_sig,
                        stringify!($in_name),
                        $in_type
                        $(, $( $in_key = $in_val ),* )?
                    ),
                )*
                $(
                    $crate::__make_out_port_spec!(
                        $out_sig,
                        stringify!($out_name),
                        $out_type
                        $(, $( $out_key = $out_val ),* )?
                    ),
                )*
            ];

            fn new(env: &$crate::ModuleEnv) -> Self {
                let mut instance = Self {
                    $( $state_field: ::core::default::Default::default(), )*
                };
                instance.init(env);
                instance
            }
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __in_port_type {
    (@, Option<$t:ty>, $lt:lifetime) => { Option<$t> };
    (@, $t:ty, $lt:lifetime)         => { $t };
    (~, $t:ty, $lt:lifetime)         => { &$lt [$t] };
    (event, $t:ty, $lt:lifetime)     => { &$lt [$t] };
}


#[doc(hidden)]
#[macro_export]
macro_rules! __out_port_type {
    (@, $t:ty, $lt:lifetime) => { &$lt mut $t };
    (~, $t:ty, $lt:lifetime) => { &$lt mut [$t] };
    (event, $t:ty, $lt:lifetime) => { &$lt mut Vec<$t> };
}


#[doc(hidden)]
#[macro_export]
macro_rules! __make_in_port_spec {
    // Parameter (@)
    (@, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )? $(, enum_variants = $variants:expr )?) => {
        $crate::PortSpec {
            name: $name,
            dir: $crate::PortDir::Input,
            kind: $crate::PortKind::Parameter,
            rate: $crate::__opt_rate!($( $rate )?),
            range: $crate::__opt_range!($( $range )?),
            port_type: $crate::__make_port_type!($ty $(, $variants )?),
        }
    };
    // Signal (~)
    (~, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )?) => {
        $crate::PortSpec {
            name: $name,
            dir: $crate::PortDir::Input,
            kind: $crate::PortKind::Signal,
            rate: $crate::__opt_rate_audio!($( $rate )?),
            range: $crate::__opt_range!($( $range )?),
            port_type: $crate::PortType::of::<$ty>(),
        }
    };
    // Event (event)
    (event, $name:expr, $ty:ty) => {
        $crate::PortSpec {
            name: $name,
            dir: $crate::PortDir::Input,
            kind: $crate::PortKind::Event,
            rate: None,
            range: None,
            port_type: $crate::PortType::of::<$ty>(),
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __make_out_port_spec {
    // Parameter (@)
    (@, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )? $(, enum_variants = $variants:expr )?) => {
        $crate::PortSpec {
            name: $name,
            dir: $crate::PortDir::Output,
            kind: $crate::PortKind::Parameter,
            rate: $crate::__opt_rate!($( $rate )?),
            range: $crate::__opt_range!($( $range )?),
            port_type: $crate::__make_port_type!($ty $(, $variants )?),
        }
    };
    // Signal (~)
    (~, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )?) => {
        $crate::PortSpec {
            name: $name,
            dir: $crate::PortDir::Output,
            kind: $crate::PortKind::Signal,
            rate: $crate::__opt_rate_audio!($( $rate )?),
            range: $crate::__opt_range!($( $range )?),
            port_type: $crate::PortType::of::<$ty>(),
        }
    };
    // Event (event)
    (event, $name:expr, $ty:ty) => {
        $crate::PortSpec {
            name: $name,
            dir: $crate::PortDir::Output,
            kind: $crate::PortKind::Event,
            rate: None,
            range: None,
            port_type: $crate::PortType::of::<$ty>(),
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __make_port_type {
    ($ty:ty) => {
        $crate::PortType::of::<$ty>()
    };
    ($ty:ty, $variants:expr) => {
        $crate::PortType::of_enum::<$ty>($variants)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __opt_rate {
    () => { Some($crate::Rate::ControlRate) };
    ($rate:expr) => { Some($rate) };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __opt_rate_audio {
    () => { Some($crate::Rate::AudioRate) };
    ($rate:expr) => { Some($rate) };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __opt_range {
    () => { None };
    ($range:expr) => { Some($range) };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __opt_req_false {
    () => { false };
    ($req:expr) => { $req };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __opt_req_true {
    () => { true };
    ($req:expr) => { $req };
}
