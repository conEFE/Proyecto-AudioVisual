//! `av-live`: visualizador de audio en vivo → OSC → TouchDesigner.
//!
//! Ejemplos:
//!   av-live                         micrófono / entrada por defecto
//!   av-live --list-devices          ver dispositivos de audio
//!   av-live --device 3 --meter      usar el dispositivo 3 y ver medidores
//!   av-live --file cancion.wav      probar con un archivo como si fuera en vivo

use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser;

use audiovisual::input;
use audiovisual::{Config, Features, LiveAnalyzer, OscSender};

const METER_KEYS: [&str; 11] = [
    "mix/level",
    "band/sub",
    "band/bass",
    "band/mid",
    "band/high",
    "drums/kick",
    "drums/snare",
    "drums/hihat",
    "bass",
    "lead",
    "tempo/beat",
];

#[derive(Parser, Debug)]
#[command(version, about = "Visualizador de audio en vivo → OSC → TouchDesigner")]
struct Args {
    /// Archivo YAML que se aplica encima de la configuración incluida (y de ./config.yaml)
    #[arg(long)]
    config: Option<PathBuf>,
    /// Lista dispositivos de audio y sale
    #[arg(long)]
    list_devices: bool,
    /// Índice o parte del nombre del dispositivo de entrada
    #[arg(long)]
    device: Option<String>,
    /// Cantidad de canales a capturar
    #[arg(long)]
    channels: Option<u16>,
    /// Captura lo que suena en la PC (Spotify, YouTube…). Sólo Windows
    #[arg(long)]
    sistema: bool,
    /// Analiza un archivo (WAV, MP3, FLAC) simulando entrada en vivo
    #[arg(long)]
    file: Option<PathBuf>,
    /// Con --file: no reproducir por parlantes
    #[arg(long)]
    no_playback: bool,
    /// Con --file --no-playback: procesar lo más rápido posible (pruebas / benchmark)
    #[arg(long)]
    fast: bool,
    /// Con --file: repetir en bucle
    #[arg(long = "loop")]
    looping: bool,
    /// IP destino OSC
    #[arg(long)]
    host: Option<String>,
    /// Puerto OSC
    #[arg(long)]
    port: Option<u16>,
    /// Muestra medidores en la terminal
    #[arg(long)]
    meter: bool,
}

fn meter_line(f: &Features) -> String {
    let mut s = String::new();
    for key in METER_KEYS {
        let v = f.get(key).unwrap_or(0.0).clamp(0.0, 1.0);
        let label: String = key.rsplit('/').next().unwrap_or(key).chars().take(5).collect();
        let bar = "█".repeat((v * 8.0) as usize);
        s.push_str(&format!("{label:>5} {bar:<8} "));
    }
    s.push_str(&format!("{:5.1}bpm", f.get("tempo/bpm").unwrap_or(0.0)));
    s
}

fn device_from_config(v: &Option<serde_yaml::Value>) -> Option<String> {
    match v {
        Some(serde_yaml::Value::Number(n)) => Some(n.to_string()),
        Some(serde_yaml::Value::String(s)) => Some(s.clone()),
        _ => None,
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.list_devices {
        print!("{}", input::list_devices()?);
        return Ok(());
    }
    let cfg = Config::load(args.config.as_deref())?;

    let inputs = cfg.audio.inputs.clone().unwrap_or_default();
    let mut channels = args.channels.unwrap_or(cfg.audio.channels);
    if let Some(max) = inputs.iter().map(|i| i.channel).max() {
        channels = channels.max(max);
    }

    let mut source = match &args.file {
        Some(path) => input::file_input(path, !args.no_playback, !args.fast, args.looping)?,
        None if args.sistema => input::system_audio_input()?,
        None => {
            let device = args.device.clone().or_else(|| device_from_config(&cfg.audio.device));
            input::live_input(device.as_deref(), channels)
                .context("no se pudo abrir la entrada de audio (prueba con --list-devices y --device N)")?
        }
    };
    let sr = source.sample_rate;
    let hop = cfg.analysis.hop_size;
    let ch = source.channels;

    let host = args.host.clone().unwrap_or_else(|| cfg.osc.host.clone());
    let port = args.port.unwrap_or(cfg.osc.port);
    let mut sender = OscSender::new(&host, port, &cfg.osc.prefix, cfg.osc.send_fps)?;

    let mut mix = LiveAnalyzer::new(sr, &cfg);
    let mix_group = sender.register("", &mix.features);
    let mut per_channel: Vec<(usize, LiveAnalyzer, usize)> = Vec::new();
    for i in &inputs {
        let idx = i.channel.saturating_sub(1) as usize;
        if idx >= ch {
            eprintln!(
                "aviso: el canal {} ({}) no existe en esta entrada ({ch} canales)",
                i.channel, i.name
            );
            continue;
        }
        let an = LiveAnalyzer::new(sr, &cfg);
        let g = sender.register(&format!("ch/{}", i.name), &an.features);
        per_channel.push((idx, an, g));
    }

    eprintln!(
        "Enviando OSC a {host}:{port} ({sr} Hz, bloque {hop} = {:.1} ms). Ctrl+C para salir.",
        1000.0 * hop as f32 / sr as f32
    );
    if !per_channel.is_empty() {
        let list: Vec<String> = inputs.iter().map(|i| format!("{}→/ch/{}", i.channel, i.name)).collect();
        eprintln!("Canales: {}", list.join(", "));
    }

    let mut block = vec![0.0f32; hop * ch];
    let mut mono = vec![0.0f32; hop];
    let mut chan = vec![0.0f32; hop];
    let mut last_meter = Instant::now();
    let started = Instant::now();
    let mut frames = 0u64;
    let mut busy = Duration::ZERO;
    while source.read_block(&mut block) {
        let t0 = Instant::now();
        for (i, m) in mono.iter_mut().enumerate() {
            *m = block[i * ch..(i + 1) * ch].iter().sum::<f32>() / ch as f32;
        }
        let f = mix.process(&mono);
        sender.send(mix_group, f);
        for (idx, an, g) in &mut per_channel {
            for (i, c) in chan.iter_mut().enumerate() {
                *c = block[i * ch + *idx];
            }
            sender.send(*g, an.process(&chan));
        }
        sender.flush(false)?;
        busy += t0.elapsed();
        frames += 1;
        if args.meter && last_meter.elapsed() > Duration::from_millis(66) {
            last_meter = Instant::now();
            eprint!("\r{}", meter_line(&mix.features));
            let _ = std::io::stderr().flush();
        }
    }
    sender.flush(true)?;
    if args.meter {
        eprintln!();
    }
    if frames > 0 {
        let audio_secs = frames as f64 * hop as f64 / sr as f64;
        eprintln!(
            "Fin: {frames} bloques ({audio_secs:.1} s de audio) en {:.2} s; análisis {:.3} ms por bloque.",
            started.elapsed().as_secs_f64(),
            busy.as_secs_f64() * 1000.0 / frames as f64
        );
    }
    Ok(())
}
