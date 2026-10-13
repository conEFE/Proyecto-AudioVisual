//! Motor de audio: corre en su propio hilo, analiza el audio y deja los valores
//! listos para que la ventana los lea en cada cuadro.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use audiovisual::{Config, LiveAnalyzer, OscSender, input};

#[derive(Debug, Clone, PartialEq)]
pub enum InputKind {
    /// Entrada de audio en vivo (`None` = la del sistema por defecto).
    Live(Option<usize>),
    /// Archivo reproducido por los parlantes.
    File(PathBuf),
    /// Canción de demostración generada por la app.
    Demo,
}

#[derive(Debug, Clone, PartialEq)]
pub struct OscTarget {
    pub host: String,
    pub port: u16,
}

#[derive(Default)]
pub struct Shared {
    pub names: Vec<String>,
    pub values: Vec<f32>,
    pulse: Vec<bool>,
    consumed: bool,
    pub status: String,
    pub error: Option<String>,
    pub finished: bool,
}

impl Shared {
    /// Copia los valores actuales. Los golpes se mantienen hasta que alguien los lee.
    pub fn take(&mut self, out: &mut Vec<f32>) {
        out.clear();
        out.extend_from_slice(&self.values);
        self.consumed = true;
    }

    fn publish(&mut self, values: &[f32], pulse: &[bool]) {
        if self.values.len() != values.len() || self.consumed {
            self.values.clear();
            self.values.extend_from_slice(values);
            self.pulse = pulse.to_vec();
        } else {
            for (i, &v) in values.iter().enumerate() {
                self.values[i] = if self.pulse[i] { self.values[i].max(v) } else { v };
            }
        }
        self.consumed = false;
    }
}

pub struct Engine {
    pub shared: Arc<Mutex<Shared>>,
    stop: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Engine {
    pub fn start(kind: InputKind, osc: Option<OscTarget>, cfg: Config) -> Self {
        let shared = Arc::new(Mutex::new(Shared::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (sh, st) = (shared.clone(), stop.clone());
        let handle = thread::spawn(move || {
            if let Err(e) = run(kind, osc, cfg, &sh, &st) {
                let mut s = sh.lock().unwrap();
                s.error = Some(format!("{e:#}"));
                s.status = "sin audio".into();
            }
        });
        Self {
            shared,
            stop,
            handle: Some(handle),
        }
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Reproduce por los parlantes; si no hay salida de audio, igual entrega el audio en tiempo real.
fn play(data: input::AudioData) -> anyhow::Result<input::Source> {
    let copy = input::AudioData {
        samples: data.samples.clone(),
        ..data
    };
    input::data_input(data, true, true, true).or_else(|_| input::data_input(copy, false, true, true))
}

fn run(
    kind: InputKind,
    osc: Option<OscTarget>,
    cfg: Config,
    shared: &Mutex<Shared>,
    stop: &AtomicBool,
) -> anyhow::Result<()> {
    let names = input::input_device_names();
    let (mut source, label) = match &kind {
        InputKind::Live(idx) => {
            let dev = idx.map(|i| i.to_string());
            let src = input::live_input(dev.as_deref(), cfg.audio.channels)?;
            let name = idx
                .and_then(|i| names.get(i).cloned())
                .unwrap_or_else(|| "entrada por defecto".into());
            (src, format!("En vivo: {name}"))
        }
        InputKind::File(path) => {
            let data = input::decode_file(path)?;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            (play(data)?, format!("Archivo: {name}"))
        }
        InputKind::Demo => (play(crate::demo::song(32.0))?, "Canción de demostración".to_string()),
    };
    let ch = source.channels;
    let hop = cfg.analysis.hop_size;
    let mut analyzer = LiveAnalyzer::new(source.sample_rate, &cfg);
    let mut sender = match &osc {
        Some(t) => Some(OscSender::new(&t.host, t.port, &cfg.osc.prefix, cfg.osc.send_fps)?),
        None => None,
    };
    let group = sender.as_mut().map(|s| s.register("", &analyzer.features));
    {
        let mut s = shared.lock().unwrap();
        s.names = analyzer.features.names.clone();
        s.status = format!("{label} · {} Hz", source.sample_rate);
        s.error = None;
    }

    let mut block = vec![0.0f32; hop * ch];
    let mut mono = vec![0.0f32; hop];
    while !stop.load(Ordering::Acquire) {
        match source.try_read_block(&mut block) {
            Some(true) => {
                for (i, m) in mono.iter_mut().enumerate() {
                    *m = block[i * ch..(i + 1) * ch].iter().sum::<f32>() / ch as f32;
                }
                let f = analyzer.process(&mono);
                if let (Some(s), Some(g)) = (sender.as_mut(), group) {
                    s.send(g, f);
                    let _ = s.flush(false);
                }
                shared.lock().unwrap().publish(&f.values, &f.pulse);
            }
            Some(false) => {
                shared.lock().unwrap().finished = true;
                break;
            }
            None => thread::sleep(Duration::from_millis(1)),
        }
    }
    Ok(())
}
