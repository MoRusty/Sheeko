use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, FromSample, OutputCallbackInfo, SampleFormat, SizedSample, Stream, StreamConfig};
use hecs::World;
use std::time::Duration;
use tracing::info;

use sheeko::ecs::components::{AudioSource, Identity, OwnedBy};

fn main() -> Result<()> {
    sheeko::telemetry::init();

    tag_local_device_as_audio_source();

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .context("no default output device")?;
    info!(id = %device.id()?, "using output device");

    let config = device.default_output_config()?;
    info!(?config, "output config");
    let stream_config: StreamConfig = config.into();

    let stream = match config.sample_format() {
        SampleFormat::F32 => build_tone_stream::<f32>(&device, stream_config)?,
        SampleFormat::I16 => build_tone_stream::<i16>(&device, stream_config)?,
        other => anyhow::bail!("unsupported sample format {other:?}"),
    };

    stream.play()?;
    info!("playing 440Hz tone (Ctrl+C to stop)");
    loop {
        std::thread::sleep(Duration::from_secs(60));
    }
}

fn build_tone_stream<T>(device: &Device, config: StreamConfig) -> Result<Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let sample_rate = config.sample_rate as f32;
    let channels = config.channels as usize;

    let mut sample_clock = 0f32;
    let mut next_value = move || {
        sample_clock = (sample_clock + 1.0) % sample_rate;
        (sample_clock * 440.0 * 2.0 * std::f32::consts::PI / sample_rate).sin()
    };

    let err_fn = |err| tracing::warn!(%err, "stream error");

    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &OutputCallbackInfo| {
            for frame in data.chunks_mut(channels) {
                let value = T::from_sample(next_value());
                for sample in frame.iter_mut() {
                    *sample = value;
                }
            }
        },
        err_fn,
        None,
    )?;
    Ok(stream)
}

/// Exercises the capability-component pattern as soon as audio exists, ahead
/// of rooms/forwarding: a throwaway local `World` gets a User entity and a
/// Device entity owned by it, and the Device is tagged `AudioSource` the
/// moment it starts producing audio. This is a local, single-process demo —
/// it does not register with a running `gateway`; the real cross-process
/// version (and moving `AudioSource` between two Devices of one User) lands
/// in M5.
fn tag_local_device_as_audio_source() {
    let mut world = World::new();
    let user = world.spawn((Identity {
        username: "local".into(),
    },));
    let device = world.spawn((OwnedBy(user),));
    world.insert_one(device, AudioSource).unwrap();

    let tagged = world.get::<&AudioSource>(device).is_ok();
    info!(?device, tagged, "local Device entity tagged AudioSource");
}
