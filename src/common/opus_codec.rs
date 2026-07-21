use audiopus::{
    Application, Channels, Result, SampleRate,
    coder::{Decoder, Encoder},
};

/// This project fixes Opus to 48kHz mono, 20ms frames (960 samples) per
/// CLAUDE.md's audio spec.
pub const SAMPLE_RATE: SampleRate = SampleRate::Hz48000;
pub const CHANNELS: Channels = Channels::Mono;
pub const FRAME_SIZE: usize = 960;

pub fn new_encoder() -> Result<Encoder> {
    Encoder::new(SAMPLE_RATE, CHANNELS, Application::Voip)
}

pub fn new_decoder() -> Result<Decoder> {
    Decoder::new(SAMPLE_RATE, CHANNELS)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// M2.0 risk gate: proves the Opus C library actually links (via
    /// pkg-config against the system libopus, or the vendored build as a
    /// fallback) before any cpal/mic code is written on top of it.
    #[test]
    fn opus_library_links_and_encodes_silence() {
        let encoder = new_encoder().expect("failed to construct Opus encoder");
        let silence = [0i16; FRAME_SIZE];
        let mut out = [0u8; 512];
        let len = encoder
            .encode(&silence, &mut out)
            .expect("failed to encode a silent frame");
        assert!(len > 0);
    }

    /// Task 6: full encode/decode round trip on a synthetic sine wave.
    ///
    /// Opus has algorithmic delay (encoder look-ahead), so a single frame's
    /// decoded output is time-shifted relative to its own input — comparing
    /// sample-by-sample against the first frame is not meaningful. Instead
    /// this feeds several consecutive frames through the same encoder/decoder
    /// (so codec state settles into steady state) and checks the *energy*
    /// (RMS) of the last frame against the sine wave's analytically known
    /// RMS (`amplitude / sqrt(2)`), which sidesteps the alignment issue.
    #[test]
    fn round_trip_sine_wave_is_close_to_original() {
        let encoder = new_encoder().unwrap();
        let mut decoder = new_decoder().unwrap();

        let freq = 440.0_f64;
        let sample_rate = 48_000.0_f64;
        let amplitude = i16::MAX as f64 * 0.5;
        let num_frames = 5;

        let mut last_output = [0i16; FRAME_SIZE];
        for frame_idx in 0..num_frames {
            let input: Vec<i16> = (0..FRAME_SIZE)
                .map(|i| {
                    let sample_index = frame_idx * FRAME_SIZE + i;
                    let t = sample_index as f64 / sample_rate;
                    ((2.0 * std::f64::consts::PI * freq * t).sin() * amplitude) as i16
                })
                .collect();

            let mut encoded = [0u8; 512];
            let encoded_len = encoder.encode(&input, &mut encoded).unwrap();

            let decoded_len = decoder
                .decode(Some(&encoded[..encoded_len]), &mut last_output[..], false)
                .unwrap();
            assert_eq!(decoded_len, FRAME_SIZE);
        }

        let rms = |samples: &[i16]| -> f64 {
            (samples.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / samples.len() as f64)
                .sqrt()
        };

        let expected_rms = amplitude / std::f64::consts::SQRT_2;
        let actual_rms = rms(&last_output);
        let relative_error = (actual_rms - expected_rms).abs() / expected_rms;
        assert!(
            relative_error < 0.1,
            "decoded steady-state RMS too far from expected: actual={actual_rms}, expected={expected_rms}"
        );
    }
}
