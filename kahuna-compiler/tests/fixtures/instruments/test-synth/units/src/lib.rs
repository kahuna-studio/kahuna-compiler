pub mod oscillators {
    use kahuna_core::{Audio, Control};
    use kahuna_unit::kahuna_module;

    #[kahuna_module]
    pub struct SineOsc {
        phase: f32,

        #[input]
        frequency: Control,

        #[output]
        out: Audio,
    }
}
