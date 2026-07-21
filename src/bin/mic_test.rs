use anyhow::{Context, Result};
use cpal::Sample;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;
use tracing::{info, warn};

fn main() -> Result<()> {
    sheeko::telemetry::init();

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("no default input device")?;
    info!(id = %device.id()?, "using input device");

    let config = device.default_input_config()?;
    info!(?config, "input config");

    let level = Arc::new(AtomicU32::new(0.0f32.to_bits()));
    let stream_config: StreamConfig = config.into();

    let err_fn = |err| warn!(%err, "stream error");

    let stream = match config.sample_format() {
        SampleFormat::F32 => {
            let level = level.clone();
            device.build_input_stream(
                stream_config,
                move |data: &[f32], _| update_level(&level, data.iter().copied()),
                err_fn,
                None,
            )?
        }
        SampleFormat::I16 => {
            let level = level.clone();
            device.build_input_stream(
                stream_config,
                move |data: &[i16], _| update_level(&level, data.iter().map(|s| s.to_sample())),
                err_fn,
                None,
            )?
        }
        other => anyhow::bail!("unsupported sample format {other:?}"),
    };

    stream.play()?;
    info!("listening — speak into the mic (Ctrl+C to stop)");

    loop {
        std::thread::sleep(Duration::from_millis(200));
        let rms = f32::from_bits(level.load(Ordering::Relaxed));
        let bar_len = (rms * 200.0).min(50.0) as usize;
        println!("[{:<50}] {rms:.4}", "=".repeat(bar_len));
    }
}

/// Computes RMS amplitude over one callback's worth of samples and stores it
/// (as raw f32 bits, since `AtomicF32` doesn't exist) for the meter loop to print.
fn update_level(level: &AtomicU32, samples: impl Iterator<Item = f32>) {
    let mut sum_sq = 0.0f32;
    let mut n = 0u32;
    for s in samples {
        sum_sq += s * s;
        n += 1;
    }
    if n > 0 {
        let rms = (sum_sq / n as f32).sqrt();
        level.store(rms.to_bits(), Ordering::Relaxed);
    }
}
