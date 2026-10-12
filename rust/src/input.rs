//! Fuentes de audio: entrada en vivo (micrófono / interfaz / loopback) o archivo simulando vivo.
//!
//! El audio llega por un buffer circular sin bloqueos (`rtrb`): el callback de audio
//! sólo copia muestras, nunca reserva memoria ni espera, y el análisis corre en otro hilo.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, Stream, StreamConfig};
use rtrb::{Consumer, Producer, RingBuffer};

/// Audio entrelazado leído de un archivo.
pub struct AudioData {
    pub samples: Vec<f32>,
    pub channels: usize,
    pub sample_rate: u32,
}

/// Decodifica WAV, MP3 o FLAC a f32 entrelazado.
pub fn decode_file(path: &Path) -> Result<AudioData> {
    use symphonia::core::codecs::audio::AudioDecoderOptions;
    use symphonia::core::errors::Error;
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;

    let file = std::fs::File::open(path).with_context(|| format!("no se pudo abrir {}", path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mut format = symphonia::default::get_probe()
        .probe(&hint, mss, FormatOptions::default(), MetadataOptions::default())
        .context("formato de audio no soportado (usa WAV, MP3 o FLAC)")?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| anyhow!("el archivo no tiene audio"))?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or_else(|| anyhow!("pista sin parámetros de audio"))?;
    let mut decoder = symphonia::default::get_codecs().make_audio_decoder(params, &AudioDecoderOptions::default())?;

    let mut samples = Vec::new();
    let mut chunk: Vec<f32> = Vec::new();
    let mut channels = 0;
    let mut sample_rate = 0;
    while let Some(packet) = format.next_packet()? {
        if packet.track_id != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(buf) => {
                let spec = buf.spec();
                channels = spec.channels().count();
                sample_rate = spec.rate();
                chunk.resize(buf.samples_interleaved(), 0.0);
                buf.copy_to_slice_interleaved(&mut chunk);
                samples.extend_from_slice(&chunk);
            }
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }
    if channels == 0 {
        bail!("no se pudo decodificar audio de {}", path.display());
    }
    Ok(AudioData {
        samples,
        channels,
        sample_rate,
    })
}

/// Remuestreo lineal simple (suficiente para análisis y monitoreo).
pub fn resample(data: &AudioData, rate: u32) -> AudioData {
    if data.sample_rate == rate {
        return AudioData {
            samples: data.samples.clone(),
            channels: data.channels,
            sample_rate: rate,
        };
    }
    let ch = data.channels;
    let frames_in = data.samples.len() / ch;
    let frames_out = (frames_in as f64 * rate as f64 / data.sample_rate as f64).round() as usize;
    let step = data.sample_rate as f64 / rate as f64;
    let mut out = Vec::with_capacity(frames_out * ch);
    for i in 0..frames_out {
        let pos = i as f64 * step;
        let i0 = (pos as usize).min(frames_in.saturating_sub(1));
        let i1 = (i0 + 1).min(frames_in.saturating_sub(1));
        let t = (pos - i0 as f64) as f32;
        for c in 0..ch {
            let a = data.samples[i0 * ch + c];
            let b = data.samples[i1 * ch + c];
            out.push(a + (b - a) * t);
        }
    }
    AudioData {
        samples: out,
        channels: ch,
        sample_rate: rate,
    }
}

/// Una fuente de audio en marcha. Se leen bloques con [`Source::read_block`].
pub struct Source {
    consumer: Consumer<f32>,
    pub channels: usize,
    pub sample_rate: u32,
    done: Arc<AtomicBool>,
    _stream: Option<Stream>,
    _thread: Option<thread::JoinHandle<()>>,
}

impl Source {
    /// Llena `block` (frames × canales, entrelazado). Devuelve `false` cuando la fuente terminó.
    pub fn read_block(&mut self, block: &mut [f32]) -> bool {
        loop {
            if self.consumer.slots() >= block.len() {
                self.consumer.pop_entire_slice(block).expect("hay suficientes muestras");
                return true;
            }
            if self.done.load(Ordering::Acquire) && self.consumer.slots() < block.len() {
                return false;
            }
            thread::sleep(Duration::from_micros(500));
        }
    }
}

pub fn list_devices() -> Result<String> {
    let host = cpal::default_host();
    let default = host.default_input_device().map(|d| d.to_string());
    let mut out = format!("Host de audio: {:?}\n", host.id());
    for (i, dev) in host.input_devices()?.enumerate() {
        let name = dev.to_string();
        let cfg = dev
            .default_input_config()
            .map(|c| format!("{} canales, {} Hz", c.channels(), c.sample_rate()))
            .unwrap_or_else(|_| "sin configuración".into());
        let mark = if default.as_deref() == Some(name.as_str()) {
            " (por defecto)"
        } else {
            ""
        };
        out.push_str(&format!("  {i:>2}: {name} — {cfg}{mark}\n"));
    }
    Ok(out)
}

fn pick_input(device: Option<&str>) -> Result<cpal::Device> {
    let host = cpal::default_host();
    match device {
        None => host
            .default_input_device()
            .ok_or_else(|| anyhow!("no hay dispositivo de entrada por defecto")),
        Some(d) => {
            let devices: Vec<_> = host.input_devices()?.collect();
            if let Ok(i) = d.parse::<usize>() {
                return devices
                    .into_iter()
                    .nth(i)
                    .ok_or_else(|| anyhow!("no existe el dispositivo {i}"));
            }
            let needle = d.to_lowercase();
            devices
                .into_iter()
                .find(|dev| dev.to_string().to_lowercase().contains(&needle))
                .ok_or_else(|| anyhow!("ningún dispositivo contiene \"{d}\" (usa --list-devices)"))
        }
    }
}

/// Entrada en vivo desde un dispositivo. Usa la frecuencia de muestreo nativa del dispositivo.
pub fn live_input(device: Option<&str>, channels: u16) -> Result<Source> {
    let dev = pick_input(device)?;
    let default = dev.default_input_config()?;
    let rate = default.sample_rate();
    let wanted = channels.max(1);
    // primero con los canales pedidos; si el dispositivo no los acepta, con los suyos
    let mut tries = vec![wanted];
    if default.channels() != wanted {
        tries.push(default.channels());
    }
    let mut last_err = None;
    for ch in tries {
        let config = StreamConfig {
            channels: ch,
            sample_rate: rate,
            buffer_size: cpal::BufferSize::Default,
        };
        // ~1 s de margen; si el análisis se atrasa se descartan bloques en vez de acumular latencia
        let (producer, consumer) = RingBuffer::new(rate as usize * ch as usize);
        let built = match default.sample_format() {
            SampleFormat::F32 => build_input::<f32>(&dev, &config, producer),
            SampleFormat::I16 => build_input::<i16>(&dev, &config, producer),
            SampleFormat::U16 => build_input::<u16>(&dev, &config, producer),
            SampleFormat::I32 => build_input::<i32>(&dev, &config, producer),
            other => bail!("formato de muestra no soportado: {other:?}"),
        };
        match built {
            Ok(stream) => {
                stream.play()?;
                if ch != wanted {
                    eprintln!("aviso: el dispositivo no acepta {wanted} canales, usando {ch}");
                }
                eprintln!("Entrada: {dev} ({ch} canales, {rate} Hz)");
                return Ok(Source {
                    consumer,
                    channels: ch as usize,
                    sample_rate: rate,
                    done: Arc::new(AtomicBool::new(false)),
                    _stream: Some(stream),
                    _thread: None,
                });
            }
            Err(e) => last_err = Some(e),
        }
    }
    Err(last_err.expect("al menos un intento"))
}

fn build_input<T>(dev: &cpal::Device, config: &StreamConfig, mut producer: Producer<f32>) -> Result<Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let stream = dev.build_input_stream::<T, _, _>(
        *config,
        move |data: &[T], _| {
            // bloque completo o nada: así los canales nunca se desalinean
            if producer.slots() < data.len() {
                return;
            }
            for &s in data {
                let _ = producer.push(s.to_sample::<f32>());
            }
        },
        |err| eprintln!("error de audio: {err}"),
        None,
    )?;
    Ok(stream)
}

/// Reproduce un archivo como si fuera audio en vivo (útil para probar sin micrófono).
///
/// * `playback`: suena por los parlantes y el análisis va sincronizado con lo que se oye.
/// * `realtime`: sin parlantes, entrega el audio al ritmo real (si es `false`, lo más rápido posible).
pub fn file_input(path: &Path, playback: bool, realtime: bool, looping: bool) -> Result<Source> {
    let data = decode_file(path)?;
    if playback {
        return file_playback(data, looping);
    }
    let channels = data.channels;
    let rate = data.sample_rate;
    let (mut producer, consumer) = RingBuffer::new(rate as usize * channels);
    let done = Arc::new(AtomicBool::new(false));
    let done2 = done.clone();
    let chunk = 256 * channels;
    let handle = thread::spawn(move || {
        let start = Instant::now();
        let mut sent_frames = 0usize;
        loop {
            for piece in data.samples.chunks(chunk) {
                let mut rest = piece;
                while !rest.is_empty() {
                    let (_, left) = producer.push_partial_slice(rest);
                    rest = left;
                    if !rest.is_empty() {
                        if producer.is_abandoned() {
                            return;
                        }
                        thread::sleep(Duration::from_micros(500));
                    }
                }
                sent_frames += piece.len() / channels;
                if realtime {
                    let target = Duration::from_secs_f64(sent_frames as f64 / rate as f64);
                    if let Some(wait) = target.checked_sub(start.elapsed()) {
                        thread::sleep(wait);
                    }
                }
            }
            if !looping {
                break;
            }
        }
        done2.store(true, Ordering::Release);
    });
    Ok(Source {
        consumer,
        channels,
        sample_rate: rate,
        done,
        _stream: None,
        _thread: Some(handle),
    })
}

fn file_playback(data: AudioData, looping: bool) -> Result<Source> {
    let host = cpal::default_host();
    let dev = host
        .default_output_device()
        .ok_or_else(|| anyhow!("no hay salida de audio (usa --no-playback)"))?;
    let default = dev.default_output_config()?;
    let rate = default.sample_rate();
    let data = Arc::new(resample(&data, rate));
    let out_ch = default.channels() as usize;
    let config = StreamConfig {
        channels: default.channels(),
        sample_rate: rate,
        buffer_size: cpal::BufferSize::Default,
    };
    let (producer, consumer) = RingBuffer::new(rate as usize * data.channels);
    let done = Arc::new(AtomicBool::new(false));
    let pos = Arc::new(AtomicUsize::new(0));
    let stream = match default.sample_format() {
        SampleFormat::F32 => build_output::<f32>(
            &dev,
            &config,
            data.clone(),
            out_ch,
            producer,
            pos,
            done.clone(),
            looping,
        )?,
        SampleFormat::I16 => build_output::<i16>(
            &dev,
            &config,
            data.clone(),
            out_ch,
            producer,
            pos,
            done.clone(),
            looping,
        )?,
        SampleFormat::U16 => build_output::<u16>(
            &dev,
            &config,
            data.clone(),
            out_ch,
            producer,
            pos,
            done.clone(),
            looping,
        )?,
        SampleFormat::I32 => build_output::<i32>(
            &dev,
            &config,
            data.clone(),
            out_ch,
            producer,
            pos,
            done.clone(),
            looping,
        )?,
        other => bail!("formato de salida no soportado: {other:?}"),
    };
    stream.play()?;
    Ok(Source {
        consumer,
        channels: data.channels,
        sample_rate: rate,
        done,
        _stream: Some(stream),
        _thread: None,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_output<T>(
    dev: &cpal::Device,
    config: &StreamConfig,
    data: Arc<AudioData>,
    out_ch: usize,
    mut producer: Producer<f32>,
    pos: Arc<AtomicUsize>,
    done: Arc<AtomicBool>,
    looping: bool,
) -> Result<Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let in_ch = data.channels;
    let frames = data.samples.len() / in_ch;
    let stream = dev.build_output_stream::<T, _, _>(
        *config,
        move |out: &mut [T], _| {
            let mut p = pos.load(Ordering::Relaxed);
            for frame in out.chunks_mut(out_ch) {
                if p >= frames {
                    if looping {
                        p = 0;
                    } else {
                        done.store(true, Ordering::Release);
                        for s in frame.iter_mut() {
                            *s = T::from_sample(0.0f32);
                        }
                        continue;
                    }
                }
                let src = &data.samples[p * in_ch..(p + 1) * in_ch];
                for (c, s) in frame.iter_mut().enumerate() {
                    *s = T::from_sample(src[c % in_ch]);
                }
                // lo mismo que suena va al análisis (si se llena, se descarta)
                if producer.slots() >= in_ch {
                    for &v in src {
                        let _ = producer.push(v);
                    }
                }
                p += 1;
            }
            pos.store(p, Ordering::Relaxed);
        },
        |err| eprintln!("error de audio: {err}"),
        None,
    )?;
    Ok(stream)
}
