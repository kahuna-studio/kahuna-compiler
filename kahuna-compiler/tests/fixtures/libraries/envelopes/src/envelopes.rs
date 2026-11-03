pub mod adsr_envelope;

// Inline modules are scanned too. The explicit id survives renaming the struct.
pub mod followers {
    use kahuna_core::{Control, Midi, Parameter};
    use kahuna_unit::kahuna_module;

    #[kahuna_module(id = "gate-follower")]
    pub struct GateToControl {
        level: f32,

        #[input]
        gate: Parameter<bool>,

        #[input]
        notes: Midi,

        #[input(default = -12)]
        transpose: Parameter<i32>,

        #[output]
        out: Control,
    }
}
