use kahuna_core::ModuleEnv;
use kahuna_unit::{kahuna_enum, kahuna_module};
use std::sync::atomic::{AtomicU32, Ordering};

/// The first seed handed out, and the step between the seeds of consecutive instances.
const FIRST_SEED: u32 = 0x1234_5678;
const SEED_STEP: u32 = 0x9E37_79B9;
/// Scales the pink and brown filters to a similar peak level as white noise (see the tests).
const PINK_GAIN: f32 = 0.11;
const BROWN_GAIN: f32 = 3.5;

static NEXT_SEED: AtomicU32 = AtomicU32::new(FIRST_SEED);

/// The spectrum of the noise.
#[kahuna_enum]
pub enum NoiseColor {
    /// The same energy at every frequency: a bright hiss.
    White,
    /// Falls by 3 dB per octave, so every octave has the same energy: how most natural sounds balance.
    Pink,
    /// Falls by 6 dB per octave: a dark rumble.
    Brown,
}

/// Noise for percussion, wind and as a source for modulation. Without `color` it is white.
///
/// Every instance, and so every voice of a poly region, has its own sequence. The sequences depend
/// only on the order the instances are created in.
#[kahuna_module(id = "noise")]
pub struct Noise {
    random: Xorshift,
    pink: PinkFilter,
    brown: BrownFilter,

    #[input]
    color: Parameter<Option<NoiseColor>>,

    #[output]
    out: Audio,
}

impl Noise {
    pub fn init(_module_env: &ModuleEnv) -> Self {
        let seed = NEXT_SEED.fetch_add(SEED_STEP, Ordering::Relaxed);

        Self {
            random: Xorshift::new(seed),
            pink: PinkFilter::default(),
            brown: BrownFilter::default(),
        }
    }

    pub fn process<'a>(&mut self, inputs: &NoiseInputs<'a>, outputs: &mut NoiseOutputs<'a>) {
        let color = inputs.color.unwrap_or(NoiseColor::White);

        for sample in outputs.out.iter_mut() {
            let white = self.random.next_bipolar();
            *sample = match color {
                NoiseColor::White => white,
                NoiseColor::Pink => self.pink.filter(white),
                NoiseColor::Brown => self.brown.filter(white),
            };
        }
    }
}

/// Marsaglia's xorshift: a few shifts per number, and good enough for audio noise.
struct Xorshift {
    state: u32,
}

impl Xorshift {
    fn new(seed: u32) -> Self {
        // The state must never be 0, which the shifts would keep at 0.
        Self { state: seed.max(1) }
    }

    /// Uniform in [-1, 1).
    fn next_bipolar(&mut self) -> f32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

/// Paul Kellet's economy filter: three one-pole low passes and the input itself, summed, which
/// approximates -3 dB per octave over most of the audio range.
#[derive(Default)]
struct PinkFilter {
    slow: f32,
    medium: f32,
    fast: f32,
}

impl PinkFilter {
    fn filter(&mut self, white: f32) -> f32 {
        self.slow = 0.99765 * self.slow + white * 0.0990460;
        self.medium = 0.96300 * self.medium + white * 0.2965164;
        self.fast = 0.57000 * self.fast + white * 1.0526913;

        let pink = self.slow + self.medium + self.fast + white * 0.1848;
        (pink * PINK_GAIN).clamp(-1.0, 1.0)
    }
}

/// A leaky integrator of white noise: -6 dB per octave. The leak keeps it from drifting away.
#[derive(Default)]
struct BrownFilter {
    level: f32,
}

impl BrownFilter {
    fn filter(&mut self, white: f32) -> f32 {
        self.level = (self.level + 0.02 * white) / 1.02;
        (self.level * BROWN_GAIN).clamp(-1.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kahuna_core::{KahunaModule, PortDirection, StreamKind, ValueType};

    const FRAMES: usize = 65_536;

    fn run(color: Option<NoiseColor>) -> Vec<f32> {
        let env = ModuleEnv { sample_rate: 48000, max_buffer_size: FRAMES };
        let mut noise = Noise::new(&env);

        let mut out = vec![0.0; FRAMES];
        let inputs = NoiseInputs::new(color);
        let mut outputs = NoiseOutputs::new(&mut out);
        noise.process(&inputs, &mut outputs);

        out
    }

    fn mean(samples: &[f32]) -> f32 {
        samples.iter().sum::<f32>() / samples.len() as f32
    }

    fn root_mean_square(samples: &[f32]) -> f32 {
        (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
    }

    /// Close to 0 for white noise, close to 1 when the signal moves slowly: a measure of how dark it is.
    fn neighbour_correlation(samples: &[f32]) -> f32 {
        let products: f32 = samples.windows(2).map(|pair| pair[0] * pair[1]).sum();
        products / samples.iter().map(|sample| sample * sample).sum::<f32>()
    }

    #[test]
    fn test_id() {
        assert_eq!(Noise::ID, "noise");
    }

    #[test]
    fn test_ports_reflection() {
        let names: Vec<_> = Noise::PORTS.iter().map(|port| port.name).collect();
        assert_eq!(names, ["color", "out"]);

        let color = &Noise::PORTS[0];
        assert_eq!(color.direction, PortDirection::Input);
        assert_eq!(color.stream_kind, StreamKind::Parameter);
        assert_eq!(color.required, Some(false));
        assert!(color.value_type.matches(&ValueType::of::<NoiseColor>()));

        assert_eq!(Noise::PORTS[1].direction, PortDirection::Output);
    }

    #[test]
    fn test_without_a_color_the_noise_is_white() {
        let noise = run(None);

        assert!(noise.iter().all(|sample| (-1.0..1.0).contains(sample)));
        assert!(mean(&noise).abs() < 0.02);
        assert!((root_mean_square(&noise) - 1.0 / 3.0_f32.sqrt()).abs() < 0.02, "uniform noise has an rms of 1/sqrt(3)");
        assert!(neighbour_correlation(&noise).abs() < 0.05);
    }

    #[test]
    fn test_the_colors_get_darker_from_white_to_pink_to_brown() {
        let white = neighbour_correlation(&run(Some(NoiseColor::White)));
        let pink = neighbour_correlation(&run(Some(NoiseColor::Pink)));
        let brown = neighbour_correlation(&run(Some(NoiseColor::Brown)));

        assert!(white.abs() < 0.05, "white: {white}");
        assert!((0.6..0.9).contains(&pink), "pink: {pink}");
        assert!(brown > 0.95, "brown: {brown}");
    }

    #[test]
    fn test_pink_and_brown_stay_within_range_and_audible() {
        for color in [NoiseColor::Pink, NoiseColor::Brown] {
            let noise = run(Some(color));

            assert!(noise.iter().all(|sample| (-1.0..=1.0).contains(sample)), "{color:?}");
            assert!(mean(&noise).abs() < 0.05, "{color:?}");
            assert!((0.1..0.4).contains(&root_mean_square(&noise)), "{color:?}: {}", root_mean_square(&noise));
        }
    }

    #[test]
    fn test_instances_have_their_own_sequences() {
        assert_ne!(run(None), run(None));
    }
}
