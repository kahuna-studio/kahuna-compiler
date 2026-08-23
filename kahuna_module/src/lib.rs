pub mod port_manifest;

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
#[derive(Debug, Clone, PartialEq)]
pub struct PortSpec {
    pub name: &'static str,
    pub direction: PortDir,
    pub stream_type: PortKind,
    pub rate: Option<Rate>,
    pub range: Option<Range>,
    /// Derived from the field's type: `Option<T>` => false, plain `T` => true.
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
//
// `inputs { ... }` is parsed token-by-token (tt-muncher) BEFORE any `Option<T>`
// gets sealed into a `:ty` fragment. This is required because macro_rules
// cannot pattern-match inside an already-sealed `ty` fragment — see
// __munch_inputs! below. `outputs { ... }` doesn't have this problem since it
// has no Option-detection requirement, so it keeps the simpler direct form.
#[macro_export]
macro_rules! kahuna_module {
    (
        $(#[$meta:meta])*
        struct $name:ident {
            $( $state_field:ident : $state_type:ty ),* $(,)?
        }

        inputs {
            $($in_tokens:tt)*
        }

        outputs {
            $( $out_sig:tt $out_name:ident : $out_type:ty $( [ $( $out_key:ident = $out_val:expr ),* $(,)? ] )? ),* $(,)?
        }
    ) => {
        $crate::__munch_inputs! {
            @meta [$(#[$meta])*]
            @name [$name]
            @state [$( $state_field : $state_type ),*]
            @outputs [$( $out_sig $out_name : $out_type $( [ $( $out_key = $out_val ),* ] )? ),*]
            @fields []
            @specs []
            @ctor_params []
            @ctor_inits []
            @rest [$($in_tokens)*]
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __munch_inputs {
    // ---- Base case: no more input tokens left. Emit everything. ----
    (
        @meta [$(#[$meta:meta])*]
        @name [$name:ident]
        @state [$( $state_field:ident : $state_type:ty ),* $(,)?]
        @outputs [$( $out_sig:tt $out_name:ident : $out_type:ty $( [ $( $out_key:ident = $out_val:expr ),* $(,)? ] )? ),* $(,)?]
        @fields [$($in_field:tt)*]
        @specs [$($in_spec:tt)*]
        @ctor_params [$($ctor_param:tt)*]
        @ctor_inits [$($ctor_init:tt)*]
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
            pub fn new($($ctor_param)*) -> Self {
                Self { $($ctor_init)* _phantom: ::std::marker::PhantomData }
            }
        }

        pub struct Outputs<'a> {
            $(
                pub $out_name : $crate::__out_port_type!($out_sig, $out_type, 'a),
            )*
            pub _phantom: ::std::marker::PhantomData<&'a ()>,
        }

        impl<'a> Outputs<'a> {
            pub fn new($( $out_name : $crate::__out_port_type!($out_sig, $out_type, 'a) ),*) -> Self {
                Self { $( $out_name, )* _phantom: ::std::marker::PhantomData }
            }
        }

        impl $crate::KahunaModule for $name {
            const PORTS: &'static [$crate::PortSpec] = &[
                $($in_spec)*
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
                Self::init(env)
            }
        }
    };

    // ---- Parameter, optional: `@name: Option<T> [ ... ],` ----
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*] @outputs [$($out:tt)*]
        @fields [$($fld:tt)*] @specs [$($spec:tt)*]
        @ctor_params [$($cp:tt)*] @ctor_inits [$($ci:tt)*]
        @rest [
            @ $pname:ident : Option<$pt:ty>
            $( [ $( $key:ident = $val:expr ),* $(,)? ] )?
            , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_inputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*] @outputs [$($out)*]
            @fields [ $($fld)* pub $pname : Option<$pt>, ]
            @specs [ $($spec)* $crate::__make_in_port_spec!(@opt, stringify!($pname), $pt $(, $( $key = $val ),* )?), ]
            @ctor_params [ $($cp)* $pname : Option<$pt>, ]
            @ctor_inits [ $($ci)* $pname, ]
            @rest [$($rest)*]
        }
    };

    // ---- Parameter, required: `@name: T [ ... ],` ----
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*] @outputs [$($out:tt)*]
        @fields [$($fld:tt)*] @specs [$($spec:tt)*]
        @ctor_params [$($cp:tt)*] @ctor_inits [$($ci:tt)*]
        @rest [
            @ $pname:ident : $pt:ty
            $( [ $( $key:ident = $val:expr ),* $(,)? ] )?
            , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_inputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*] @outputs [$($out)*]
            @fields [ $($fld)* pub $pname : $pt, ]
            @specs [ $($spec)* $crate::__make_in_port_spec!(@req, stringify!($pname), $pt $(, $( $key = $val ),* )?), ]
            @ctor_params [ $($cp)* $pname : $pt, ]
            @ctor_inits [ $($ci)* $pname, ]
            @rest [$($rest)*]
        }
    };

    // ---- Signal input: `~name: T [ ... ],` ----
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*] @outputs [$($out:tt)*]
        @fields [$($fld:tt)*] @specs [$($spec:tt)*]
        @ctor_params [$($cp:tt)*] @ctor_inits [$($ci:tt)*]
        @rest [
            ~ $pname:ident : $pt:ty
            $( [ $( $key:ident = $val:expr ),* $(,)? ] )?
            , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_inputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*] @outputs [$($out)*]
            @fields [ $($fld)* pub $pname : &'a [$pt], ]
            @specs [ $($spec)* $crate::__make_in_port_spec!(@sig, stringify!($pname), $pt $(, $( $key = $val ),* )?), ]
            @ctor_params [ $($cp)* $pname : &'a [$pt], ]
            @ctor_inits [ $($ci)* $pname, ]
            @rest [$($rest)*]
        }
    };

    // ---- Event input: `event name: T,` ----
    (
        @meta [$($meta:tt)*] @name [$name:ident] @state [$($sf:tt)*] @outputs [$($out:tt)*]
        @fields [$($fld:tt)*] @specs [$($spec:tt)*]
        @ctor_params [$($cp:tt)*] @ctor_inits [$($ci:tt)*]
        @rest [
            event $pname:ident : $pt:ty , $($rest:tt)*
        ]
    ) => {
        $crate::__munch_inputs! {
            @meta [$($meta)*] @name [$name] @state [$($sf)*] @outputs [$($out)*]
            @fields [ $($fld)* pub $pname : &'a [$pt], ]
            @specs [ $($spec)* $crate::__make_in_port_spec!(@evt, stringify!($pname), $pt), ]
            @ctor_params [ $($cp)* $pname : &'a [$pt], ]
            @ctor_inits [ $($ci)* $pname, ]
            @rest [$($rest)*]
        }
    };
}

// ============================================================================
// Output port type (unchanged shape — no Option-detection needed here)
// ============================================================================

#[doc(hidden)]
#[macro_export]
macro_rules! __out_port_type {
    (@, $t:ty, $lt:lifetime) => { &$lt mut $t };
    (~, $t:ty, $lt:lifetime) => { &$lt mut [$t] };
    (event, $t:ty, $lt:lifetime) => { &$lt mut Vec<$t> };
}

// ============================================================================
// PortSpec builders
// ============================================================================
//
// `__make_in_port_spec!` now takes an explicit tag (@opt / @req / @sig / @evt)
// decided by __munch_inputs! above, instead of re-deriving Option-ness from
// an already-sealed `ty` fragment (which is what was broken).

#[doc(hidden)]
#[macro_export]
macro_rules! __make_in_port_spec {
    // Parameter, optional
    (@opt, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )? $(, enum_variants = $variants:expr )?) => {
        $crate::PortSpec {
            name: $name,
            direction: $crate::PortDir::Input,
            stream_type: $crate::PortKind::Parameter,
            rate: $crate::__opt_rate!($( $rate )?),
            range: $crate::__opt_range!($( $range )?),
            required: Some(false),
            value_type: $crate::__make_port_type!($ty $(, $variants )?),
        }
    };
    // Parameter, required
    (@req, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )? $(, enum_variants = $variants:expr )?) => {
        $crate::PortSpec {
            name: $name,
            direction: $crate::PortDir::Input,
            stream_type: $crate::PortKind::Parameter,
            rate: $crate::__opt_rate!($( $rate )?),
            range: $crate::__opt_range!($( $range )?),
            required: Some(true),
            value_type: $crate::__make_port_type!($ty $(, $variants )?),
        }
    };
    // Signal
    (@sig, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )?) => {
        $crate::PortSpec {
            name: $name,
            direction: $crate::PortDir::Input,
            stream_type: $crate::PortKind::Signal,
            rate: $crate::__opt_rate_audio!($( $rate )?),
            range: $crate::__opt_range!($( $range )?),
            required: None, // Not sure if that makes sense
            value_type: $crate::PortType::of::<$ty>(),
        }
    };
    // Event
    (@evt, $name:expr, $ty:ty) => {
        $crate::PortSpec {
            name: $name,
            direction: $crate::PortDir::Input,
            stream_type: $crate::PortKind::Event,
            rate: None,
            range: None,
            required: Some(false), // Unplugged is the same as plugged, but no event passed
            value_type: $crate::PortType::of::<$ty>(),
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
            direction: $crate::PortDir::Output,
            stream_type: $crate::PortKind::Parameter,
            rate: $crate::__opt_rate!($( $rate )?),
            range: $crate::__opt_range!($( $range )?),
            required: None,
            value_type: $crate::__make_port_type!($ty $(, $variants )?),
        }
    };
    // Signal (~)
    (~, $name:expr, $ty:ty $(, rate = $rate:expr )? $(, range = $range:expr )?) => {
        $crate::PortSpec {
            name: $name,
            direction: $crate::PortDir::Output,
            stream_type: $crate::PortKind::Signal,
            rate: $crate::__opt_rate_audio!($( $rate )?),
            range: $crate::__opt_range!($( $range )?),
            required: None,
            value_type: $crate::PortType::of::<$ty>(),
        }
    };
    // Event (event)
    (event, $name:expr, $ty:ty) => {
        $crate::PortSpec {
            name: $name,
            direction: $crate::PortDir::Output,
            stream_type: $crate::PortKind::Event,
            rate: None,
            range: None,
            required: None,
            value_type: $crate::PortType::of::<$ty>(),
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