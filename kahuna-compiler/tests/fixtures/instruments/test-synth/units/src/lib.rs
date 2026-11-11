pub mod oscillators {
    use kahuna_core::{Audio, Control};
    use kahuna_unit::kahuna_module;

    // Same id as kahuna-std's sine oscillator, to cover the prefixing of colliding ids.
    #[kahuna_module(id = "sine")]
    pub struct SineOsc {
        phase: f32,

        #[input]
        frequency: Control,

        #[output]
        out: Audio,
    }
}
