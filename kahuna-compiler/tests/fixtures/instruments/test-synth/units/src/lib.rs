pub mod oscillators {
    use kahuna_core::{Audio, Control};
    use kahuna_module::kahuna_module;

    #[kahuna_module]
    pub struct SineOsc {
        phase: f32,

        #[input]
        frequency: Control,

        #[output]
        out: Audio,
    }
}
