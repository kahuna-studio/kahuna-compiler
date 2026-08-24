pub mod port_manifest;
pub mod midi;

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

// ============================================================================
// Macro for Kahuna Modules
// ============================================================================
// Both inputs and outputs are parsed as raw, unsealed tokens via tt-munchers
// (__munch_inputs! then __munch_outputs!), because Rust's macro_rules cannot
// pattern-match inside an already-sealed `ty` fragment.
// This is what lets `Option<T>` (inputs) and `Vec<T>` (event outputs) be
// detected and unwrapped to their element type.

#[macro_export]
macro_rules! kahuna_module {
    (
        $(#[$meta:meta])*
        struct $name:ident {
            $( $state_field:ident : $state_type:ty ),* $(,)?
        }

        inputs { $($in_tokens:tt)* }

        outputs { $($out_tokens:tt)* }
    ) => {
        $crate::__munch_inputs! {
            @meta [$(#[$meta])*]
            @name [$name]
            @state [$( $state_field : $state_type ),*]
            @out_tokens [$($out_tokens)*]
            @fields []
            @specs []
            @ctor_params []
            @ctor_inits []
            @rest [$($in_tokens)*]
        }
    };
}

// ---------------------------------------------------------------------------
// Input muncher — unchanged logic from before, just now hands off to
// __munch_outputs! in its base case instead of emitting directly.
// ---------------------------------------------------------------------------

#[doc(hidden)]
#[macro_export]
macro_rules! __munch_inputs {
    // Base case: inputs exhausted -> start munching outputs.
    (
        @meta [$(#[$meta:meta])*]
        @name [$name:ident]
        @state [$( $state_field:ident : $state_type:ty ),* $(,)?]
        @out_tokens [$($out_tokens:tt)*]
        @fields [$($in_field:tt)*]
        @specs [$($in_spec:tt)*]
        @ctor_params [$($ctor_param:tt)*]
        @ctor_inits [$($ctor_init:tt)*]
        @rest []
    ) => {
        $crate::__munch_outputs! {
            @meta [$(#[$meta])*]
            @name [$name]
            @state [$( $state_field : $state_type ),*]
            @in_fields [$($in_field)*]
            @in_specs [$($in_spec)*]
            @in_ctor_params [$($ctor_param)*]
            @in_ctor_inits [$($ctor_init)*]
            @out_fields []
            @out_specs []
            @out_ctor_params []
            @out_ctor_inits []
            @rest [$($out_tokens)*]
        }
    };

    // Parameter, optional: `@name: Option<T> [ ... ],`
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*] @out_tokens [$($ot:tt)*]
        @fields [$($fld:tt)*] @specs [$($spec:tt)*]
        @ctor_params [$($cp:tt)*] @ctor_inits [$($ci:tt)*]
        @rest [
            @ $pname:ident : Option<$pt:ty>
            $( [ $( $key:ident = $val:expr ),* $(,)? ] )?
            , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_inputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*] @out_tokens [$($ot)*]
            @fields [ $($fld)* pub $pname : Option<$pt>, ]
            @specs [ $($spec)* $crate::__make_in_port_spec!(@parameter_optional, stringify!($pname), $pt $(, $( $key = $val ),* )?), ]
            @ctor_params [ $($cp)* $pname : Option<$pt>, ]
            @ctor_inits [ $($ci)* $pname, ]
            @rest [$($rest)*]
        }
    };

    // Parameter, required: `@name: T [ ... ],`
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*] @out_tokens [$($ot:tt)*]
        @fields [$($fld:tt)*] @specs [$($spec:tt)*]
        @ctor_params [$($cp:tt)*] @ctor_inits [$($ci:tt)*]
        @rest [
            @ $pname:ident : $pt:ty
            $( [ $( $key:ident = $val:expr ),* $(,)? ] )?
            , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_inputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*] @out_tokens [$($ot)*]
            @fields [ $($fld)* pub $pname : $pt, ]
            @specs [ $($spec)* $crate::__make_in_port_spec!(@parameter_required, stringify!($pname), $pt $(, $( $key = $val ),* )?), ]
            @ctor_params [ $($cp)* $pname : $pt, ]
            @ctor_inits [ $($ci)* $pname, ]
            @rest [$($rest)*]
        }
    };

    // Signal, optional: `~name: Option<T> [ ... ],`
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*] @out_tokens [$($ot:tt)*]
        @fields [$($fld:tt)*] @specs [$($spec:tt)*]
        @ctor_params [$($cp:tt)*] @ctor_inits [$($ci:tt)*]
        @rest [
            ~ $pname:ident : Option<$pt:ty>
            $( [ $( $key:ident = $val:expr ),* $(,)? ] )?
            , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_inputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*] @out_tokens [$($ot)*]
            @fields [ $($fld)* pub $pname : Option<&'a [$pt]>, ]
            @specs [ $($spec)* $crate::__make_in_port_spec!(@signal_optional, stringify!($pname), $pt $(, $( $key = $val ),* )?), ]
            @ctor_params [ $($cp)* $pname : Option<&'a [$pt]>, ]
            @ctor_inits [ $($ci)* $pname, ]
            @rest [$($rest)*]
        }
    };

    // Signal, required: `~name: T [ ... ],`
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*] @out_tokens [$($ot:tt)*]
        @fields [$($fld:tt)*] @specs [$($spec:tt)*]
        @ctor_params [$($cp:tt)*] @ctor_inits [$($ci:tt)*]
        @rest [
            ~ $pname:ident : $pt:ty
            $( [ $( $key:ident = $val:expr ),* $(,)? ] )?
            , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_inputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*] @out_tokens [$($ot)*]
            @fields [ $($fld)* pub $pname : &'a [$pt], ]
            @specs [ $($spec)* $crate::__make_in_port_spec!(@signal_required, stringify!($pname), $pt $(, $( $key = $val ),* )?), ]
            @ctor_params [ $($cp)* $pname : &'a [$pt], ]
            @ctor_inits [ $($ci)* $pname, ]
            @rest [$($rest)*]
        }
    };

    // Event input: `^name: &[T],`
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*] @out_tokens [$($ot:tt)*]
        @fields [$($fld:tt)*] @specs [$($spec:tt)*]
        @ctor_params [$($cp:tt)*] @ctor_inits [$($ci:tt)*]
        @rest [
            ^ $pname:ident : &[$pt:ty] , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_inputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*] @out_tokens [$($ot)*]
            @fields [ $($fld)* pub $pname : &'a [$pt], ]
            @specs [ $($spec)* $crate::__make_in_port_spec!(@evt, stringify!($pname), $pt), ]
            @ctor_params [ $($cp)* $pname : &'a [$pt], ]
            @ctor_inits [ $($ci)* $pname, ]
            @rest [$($rest)*]
        }
    };
}

// ---------------------------------------------------------------------------
// Output muncher — new. Same discipline as inputs: match `Vec<$pt:ty>` before
// it can be sealed, so the element type is recoverable for the PortSpec.
// ---------------------------------------------------------------------------

#[doc(hidden)]
#[macro_export]
macro_rules! __munch_outputs {
    // Base case: outputs exhausted -> emit everything.
    (
        @meta [$(#[$meta:meta])*]
        @name [$name:ident]
        @state [$( $state_field:ident : $state_type:ty ),* $(,)?]
        @in_fields [$($in_field:tt)*]
        @in_specs [$($in_spec:tt)*]
        @in_ctor_params [$($in_ctor_param:tt)*]
        @in_ctor_inits [$($in_ctor_init:tt)*]
        @out_fields [$($out_field:tt)*]
        @out_specs [$($out_spec:tt)*]
        @out_ctor_params [$($out_ctor_param:tt)*]
        @out_ctor_inits [$($out_ctor_init:tt)*]
        @rest []
    ) => {
        $(#[$meta])*
        pub struct $name {
            $( $state_field : $state_type, )*
        }

        pub struct Inputs<'a> {
            $($in_field)*
            pub _phantom: ::std::marker::PhantomData<&'a ()>,
        }

        impl<'a> Inputs<'a> {
            pub fn new($($in_ctor_param)*) -> Self {
                Self { $($in_ctor_init)* _phantom: ::std::marker::PhantomData }
            }
        }

        pub struct Outputs<'a> {
            $($out_field)*
            pub _phantom: ::std::marker::PhantomData<&'a ()>,
        }

        impl<'a> Outputs<'a> {
            pub fn new($($out_ctor_param)*) -> Self {
                Self { $($out_ctor_init)* _phantom: ::std::marker::PhantomData }
            }
        }

        impl $crate::KahunaModule for $name {
            const PORTS: &'static [$crate::PortSpec] = &[
                $($in_spec)*
                $($out_spec)*
            ];

            fn new(env: &$crate::ModuleEnv) -> Self {
                Self::init(env)
            }
        }
    };

    // Parameter output: `@name: T [ ... ],`
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*]
        @in_fields [$($inf:tt)*] @in_specs [$($ins:tt)*]
        @in_ctor_params [$($icp:tt)*] @in_ctor_inits [$($ici:tt)*]
        @out_fields [$($of:tt)*] @out_specs [$($os:tt)*]
        @out_ctor_params [$($ocp:tt)*] @out_ctor_inits [$($oci:tt)*]
        @rest [
            @ $pname:ident : $pt:ty
            $( [ $( $key:ident = $val:expr ),* $(,)? ] )?
            , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_outputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*]
            @in_fields [$($inf)*] @in_specs [$($ins)*]
            @in_ctor_params [$($icp)*] @in_ctor_inits [$($ici)*]
            @out_fields [ $($of)* pub $pname : &'a mut $pt, ]
            @out_specs [ $($os)* $crate::__make_out_port_spec!(@parameter, stringify!($pname), $pt $(, $( $key = $val ),* )?), ]
            @out_ctor_params [ $($ocp)* $pname : &'a mut $pt, ]
            @out_ctor_inits [ $($oci)* $pname, ]
            @rest [$($rest)*]
        }
    };

    // Signal output: `~name: T [ ... ],`
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*]
        @in_fields [$($inf:tt)*] @in_specs [$($ins:tt)*]
        @in_ctor_params [$($icp:tt)*] @in_ctor_inits [$($ici:tt)*]
        @out_fields [$($of:tt)*] @out_specs [$($os:tt)*]
        @out_ctor_params [$($ocp:tt)*] @out_ctor_inits [$($oci:tt)*]
        @rest [
            ~ $pname:ident : $pt:ty
            $( [ $( $key:ident = $val:expr ),* $(,)? ] )?
            , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_outputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*]
            @in_fields [$($inf)*] @in_specs [$($ins)*]
            @in_ctor_params [$($icp)*] @in_ctor_inits [$($ici)*]
            @out_fields [ $($of)* pub $pname : &'a mut [$pt], ]
            @out_specs [ $($os)* $crate::__make_out_port_spec!(@signal, stringify!($pname), $pt $(, $( $key = $val ),* )?), ]
            @out_ctor_params [ $($ocp)* $pname : &'a mut [$pt], ]
            @out_ctor_inits [ $($oci)* $pname, ]
            @rest [$($rest)*]
        }
    };

    // Event output: `^name: Vec<T>,`  <- explicit "what you get" shape
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*]
        @in_fields [$($inf:tt)*] @in_specs [$($ins:tt)*]
        @in_ctor_params [$($icp:tt)*] @in_ctor_inits [$($ici:tt)*]
        @out_fields [$($of:tt)*] @out_specs [$($os:tt)*]
        @out_ctor_params [$($ocp:tt)*] @out_ctor_inits [$($oci:tt)*]
        @rest [
            ^ $pname:ident : Vec<$pt:ty> , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_outputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*]
            @in_fields [$($inf)*] @in_specs [$($ins)*]
            @in_ctor_params [$($icp)*] @in_ctor_inits [$($ici)*]
            @out_fields [ $($of)* pub $pname : &'a mut Vec<$pt>, ]
            @out_specs [ $($os)* $crate::__make_out_port_spec!(@evt, stringify!($pname), $pt), ]
            @out_ctor_params [ $($ocp)* $pname : &'a mut Vec<$pt>, ]
            @out_ctor_inits [ $($oci)* $pname, ]
            @rest [$($rest)*]
        }
    };
}

// ============================================================================
// PortSpec builders
// ============================================================================

#[doc(hidden)]
#[macro_export]
macro_rules! __make_in_port_spec {
    (@parameter_optional, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )? $(, enum_variants = $variants:expr )?) => {
        $crate::PortSpec {
            name: $name, direction: $crate::PortDir::Input, stream_type: $crate::PortKind::Parameter,
            rate: $crate::__opt_rate!($( $rate )?), range: $crate::__opt_range!($( $range )?),
            required: Some(false), value_type: $crate::__make_port_type!($ty $(, $variants )?),
        }
    };
    (@parameter_required, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )? $(, enum_variants = $variants:expr )?) => {
        $crate::PortSpec {
            name: $name, direction: $crate::PortDir::Input, stream_type: $crate::PortKind::Parameter,
            rate: $crate::__opt_rate!($( $rate )?), range: $crate::__opt_range!($( $range )?),
            required: Some(true), value_type: $crate::__make_port_type!($ty $(, $variants )?),
        }
    };
    (@signal_optional, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )?) => {
        $crate::PortSpec {
            name: $name, direction: $crate::PortDir::Input, stream_type: $crate::PortKind::Signal,
            rate: $crate::__opt_rate_audio!($( $rate )?), range: $crate::__opt_range!($( $range )?),
            required: Some(false), value_type: $crate::PortType::of::<$ty>(),
        }
    };
    (@signal_required, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )?) => {
        $crate::PortSpec {
            name: $name, direction: $crate::PortDir::Input, stream_type: $crate::PortKind::Signal,
            rate: $crate::__opt_rate_audio!($( $rate )?), range: $crate::__opt_range!($( $range )?),
            required: Some(true), value_type: $crate::PortType::of::<$ty>(),
        }
    };
    (@evt, $name:expr, $ty:ty) => {
        $crate::PortSpec {
            name: $name, direction: $crate::PortDir::Input, stream_type: $crate::PortKind::Event,
            rate: None, range: None,
            required: Some(false), // unplugged == valid, empty stream
            value_type: $crate::PortType::of::<$ty>(), // element type, matches output's element type
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __make_out_port_spec {
    (@parameter, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )? $(, enum_variants = $variants:expr )?) => {
        $crate::PortSpec {
            name: $name, direction: $crate::PortDir::Output, stream_type: $crate::PortKind::Parameter,
            rate: $crate::__opt_rate!($( $rate )?), range: $crate::__opt_range!($( $range )?),
            required: None, value_type: $crate::__make_port_type!($ty $(, $variants )?),
        }
    };
    (@signal, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )?) => {
        $crate::PortSpec {
            name: $name, direction: $crate::PortDir::Output, stream_type: $crate::PortKind::Signal,
            rate: $crate::__opt_rate_audio!($( $rate )?), range: $crate::__opt_range!($( $range )?),
            required: None, value_type: $crate::PortType::of::<$ty>(),
        }
    };
    (@evt, $name:expr, $ty:ty) => {
        $crate::PortSpec {
            name: $name, direction: $crate::PortDir::Output, stream_type: $crate::PortKind::Event,
            rate: None, range: None,
            required: None,
            value_type: $crate::PortType::of::<$ty>(), // element type, extracted from Vec<T> by the muncher
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
    () => {
        Some($crate::Rate::ControlRate)
    };
    ($rate:expr) => {
        Some($rate)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __opt_rate_audio {
    () => {
        Some($crate::Rate::AudioRate)
    };
    ($rate:expr) => {
        Some($rate)
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __opt_range {
    () => {
        None
    };
    ($range:expr) => {
        Some($range)
    };
}
