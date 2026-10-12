//! Análisis de audio en tiempo real (mismo algoritmo que `visualizer/analyzer.py`).
//!
//! Por cada bloque de audio (hop) calcula:
//!   * nivel general, brillo y "ruidosidad" del sonido
//!   * energía por bandas de frecuencia (configurables)
//!   * separación armónico/percusiva (HPSS) en tiempo real
//!   * detectores de instrumentos aproximados: bombo, caja, hi-hat, bajo, melodía/voz
//!   * altura (nota) de la melodía y cromagrama (12 notas)
//!   * tempo (BPM), fase del pulso y golpe de pulso
//!
//! Todo lo que sale está normalizado a 0..1 salvo BPM, Hz y nota MIDI.
//! No reserva memoria por bloque: todos los buffers se crean al inicio.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use realfft::num_complex::Complex;
use realfft::{RealFftPlanner, RealToComplex};

use crate::config::{Config, InstrumentKind};
use crate::dsp::{AutoGain, Envelope, OnsetDetector, time_to_coef};

pub const NOTE_NAMES: [&str; 12] = ["c", "cs", "d", "ds", "e", "f", "fs", "g", "gs", "a", "as", "b"];

/// Valores de salida con nombres fijos (las direcciones OSC sin prefijo).
#[derive(Debug, Clone, Default)]
pub struct Features {
    pub names: Vec<String>,
    pub values: Vec<f32>,
    /// `true` para los pulsos (golpes): el emisor OSC no debe perderlos.
    pub pulse: Vec<bool>,
    index: HashMap<String, usize>,
}

impl Features {
    fn slot(&mut self, name: &str, pulse: bool) -> usize {
        let i = self.names.len();
        self.names.push(name.to_string());
        self.values.push(0.0);
        self.pulse.push(pulse);
        self.index.insert(name.to_string(), i);
        i
    }

    pub fn get(&self, name: &str) -> Option<f32> {
        self.index.get(name).map(|&i| self.values[i])
    }

    /// Como `get`, pero entra en pánico si el nombre no existe (útil en pruebas).
    pub fn at(&self, name: &str) -> f32 {
        self.get(name).unwrap_or_else(|| panic!("no existe el valor {name}"))
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, f32)> {
        self.names.iter().map(String::as_str).zip(self.values.iter().copied())
    }
}

type Range = std::ops::Range<usize>;

struct BandState {
    bins: Range,
    env: Envelope,
    gain: AutoGain,
    slot: usize,
}

struct InstState {
    kind: InstrumentKind,
    ranges: Vec<Range>,
    combine_all: bool,
    onset: Option<OnsetDetector>,
    env: Envelope,
    gain: AutoGain,
    slot: usize,
    level_slot: Option<usize>,
}

struct Slots {
    level: usize,
    brightness: usize,
    noisiness: usize,
    centroid: usize,
    harmonic: usize,
    percussive: usize,
    onset: usize,
    pitch_hz: usize,
    note: usize,
    pitch_class: usize,
    confidence: usize,
    chroma: [usize; 12],
    bpm: usize,
    phase: usize,
    beat: usize,
}

pub struct LiveAnalyzer {
    sr: f32,
    pub hop: usize,
    win: usize,
    nfft: usize,
    pub fps: f32,
    window: Vec<f32>,
    buffer: Vec<f32>,
    fft: Arc<dyn RealToComplex<f32>>,
    fft_in: Vec<f32>,
    fft_out: Vec<Complex<f32>>,
    fft_scratch: Vec<Complex<f32>>,
    freqs: Vec<f32>,

    mag: Vec<f32>,
    prev_mag: Vec<f32>,
    perc: Vec<f32>,
    prev_perc: Vec<f32>,
    harm: Vec<f32>,
    // HPSS: historia circular de espectros y buffers de trabajo
    kernel: usize,
    history: Vec<Vec<f32>>,
    hist_len: usize,
    hist_pos: usize,
    median_tmp: Vec<f32>,
    perc_est: Vec<f32>,
    // pitch
    score: Vec<f32>,
    comp: Vec<f32>,

    bands: Vec<BandState>,
    insts: Vec<InstState>,
    kick: Option<usize>,

    rms_env: Envelope,
    rms_gain: AutoGain,
    centroid_env: Envelope,
    flatness_env: Envelope,
    perc_env: Envelope,
    perc_gain: AutoGain,
    harm_env: Envelope,
    harm_gain: AutoGain,
    onset_all: OnsetDetector,

    pitch_bins: Range,
    pitch_conf_min: f32,
    pitch_hz: f32,
    chroma_env: Vec<Envelope>,
    bin_pitch_class: Vec<i8>,
    chroma: [f32; 12],

    tempo_min: f32,
    tempo_max: f32,
    onset_hist: VecDeque<f32>,
    onset_hist_cap: usize,
    tempo_every: u64,
    bpm: f32,
    phase: f32,
    beat_pulse: f32,
    beat_decay: f32,
    frame: u64,

    s: Slots,
    pub features: Features,
}

impl LiveAnalyzer {
    pub fn new(sample_rate: u32, cfg: &Config) -> Self {
        let a = &cfg.analysis;
        let sr = sample_rate as f32;
        let hop = a.hop_size;
        let win = a.window_size;
        let nfft = win * 2; // zero-padding: mejor resolución para la nota
        let fps = sr / hop as f32;
        let nbins = nfft / 2 + 1;
        let freqs: Vec<f32> = (0..nbins).map(|i| i as f32 * sr / nfft as f32).collect();
        let bins = |lo: f32, hi: f32| -> Range {
            let start = freqs.partition_point(|&f| f < lo);
            let end = freqs.partition_point(|&f| f < hi);
            start..end.max(start)
        };
        // ventana de Hann simétrica (igual que numpy.hanning)
        let window: Vec<f32> = (0..win)
            .map(|n| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * n as f32 / (win - 1) as f32).cos())
            .collect();

        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(nfft);
        let fft_in = fft.make_input_vec();
        let fft_out = fft.make_output_vec();
        let fft_scratch = fft.make_scratch_vec();

        let env = || Envelope::new(a.attack, a.release, fps);
        let gain = || AutoGain::new(a.autogain_seconds, fps);
        let mut f = Features::default();

        let s_level = f.slot("mix/level", false);
        let s_brightness = f.slot("mix/brightness", false);
        let s_noisiness = f.slot("mix/noisiness", false);
        let s_centroid = f.slot("mix/centroid_hz", false);

        let bands = cfg
            .bands
            .iter()
            .map(|b| BandState {
                bins: bins(b.low, b.high),
                env: env(),
                gain: gain(),
                slot: f.slot(&format!("band/{}", b.name), false),
            })
            .collect();

        let s_harmonic = f.slot("mix/harmonic", false);
        let s_percussive = f.slot("mix/percussive", false);
        let s_onset = f.slot("mix/onset", true);

        let mut kick = None;
        let insts: Vec<InstState> = cfg
            .instruments
            .iter()
            .enumerate()
            .map(|(i, (name, spec))| {
                if name == "drums/kick" {
                    kick = Some(i);
                }
                let perc = spec.kind == InstrumentKind::Percussive;
                InstState {
                    kind: spec.kind,
                    ranges: spec.ranges.iter().map(|r| bins(r[0], r[1])).collect(),
                    combine_all: spec.combine.as_deref() == Some("all"),
                    onset: perc.then(|| {
                        OnsetDetector::new(
                            fps,
                            spec.sensitivity,
                            spec.refractory,
                            spec.pulse_release,
                            spec.min_flux,
                        )
                    }),
                    env: env(),
                    gain: gain(),
                    slot: f.slot(name, perc),
                    level_slot: perc.then(|| f.slot(&format!("{name}_level"), false)),
                }
            })
            .collect();

        let s_pitch_hz = f.slot("lead/pitch_hz", false);
        let s_note = f.slot("lead/note", false);
        let s_pitch_class = f.slot("lead/pitch_class", false);
        let s_confidence = f.slot("lead/confidence", false);
        let chroma_slots = NOTE_NAMES.map(|n| f.slot(&format!("chroma/{n}"), false));
        let s_bpm = f.slot("tempo/bpm", false);
        let s_phase = f.slot("tempo/phase", false);
        let s_beat = f.slot("tempo/beat", true);

        let p = &cfg.pitch;
        let chroma_bins = bins(p.min_hz, 5000.0);
        let bin_pitch_class = freqs
            .iter()
            .enumerate()
            .map(|(i, &fr)| {
                if chroma_bins.contains(&i) {
                    let midi = 69.0 + 12.0 * (fr / 440.0).log2();
                    (midi.round() as i32).rem_euclid(12) as i8
                } else {
                    -1
                }
            })
            .collect();

        let pitch_bins = bins(p.min_hz, p.max_hz);
        let t = &cfg.tempo;
        let onset_hist_cap = (t.history_seconds * fps) as usize;
        let kernel = a.hpss_kernel;

        Self {
            sr,
            hop,
            win,
            nfft,
            fps,
            window,
            buffer: vec![0.0; win],
            fft,
            fft_in,
            fft_out,
            fft_scratch,
            freqs,
            mag: vec![0.0; nbins],
            prev_mag: vec![0.0; nbins],
            perc: vec![0.0; nbins],
            prev_perc: vec![0.0; nbins],
            harm: vec![0.0; nbins],
            kernel,
            history: vec![vec![0.0; nbins]; kernel],
            hist_len: 0,
            hist_pos: 0,
            median_tmp: vec![0.0; kernel.max(1)],
            perc_est: vec![0.0; nbins],
            score: vec![0.0; nbins],
            comp: vec![0.0; nbins],
            bands,
            insts,
            kick,
            rms_env: env(),
            rms_gain: gain(),
            centroid_env: env(),
            flatness_env: env(),
            perc_env: env(),
            perc_gain: gain(),
            harm_env: env(),
            harm_gain: gain(),
            onset_all: OnsetDetector::new(fps, a.onset_sensitivity, 0.08, 0.15, 1e-4),
            pitch_bins,
            pitch_conf_min: p.min_confidence,
            pitch_hz: 0.0,
            chroma_env: (0..12).map(|_| env()).collect(),
            bin_pitch_class,
            chroma: [0.0; 12],
            tempo_min: t.min_bpm,
            tempo_max: t.max_bpm,
            onset_hist: VecDeque::with_capacity(onset_hist_cap),
            onset_hist_cap,
            tempo_every: 1.max((t.update_seconds * fps) as u64),
            bpm: t.initial_bpm,
            phase: 0.0,
            beat_pulse: 0.0,
            beat_decay: time_to_coef(0.1, fps),
            frame: 0,
            s: Slots {
                level: s_level,
                brightness: s_brightness,
                noisiness: s_noisiness,
                centroid: s_centroid,
                harmonic: s_harmonic,
                percussive: s_percussive,
                onset: s_onset,
                pitch_hz: s_pitch_hz,
                note: s_note,
                pitch_class: s_pitch_class,
                confidence: s_confidence,
                chroma: chroma_slots,
                bpm: s_bpm,
                phase: s_phase,
                beat: s_beat,
            },
            features: f,
        }
    }

    /// Analiza un bloque mono (normalmente `hop` muestras) y actualiza `self.features`.
    pub fn process(&mut self, block: &[f32]) -> &Features {
        let n = block.len().min(self.win);
        let block = &block[block.len() - n..];
        self.buffer.copy_within(n.., 0);
        let w = self.win;
        self.buffer[w - n..].copy_from_slice(block);
        self.frame += 1;

        // FFT con ventana y zero-padding
        for (i, x) in self.fft_in.iter_mut().enumerate() {
            *x = if i < w { self.buffer[i] * self.window[i] } else { 0.0 };
        }
        self.fft
            .process_with_scratch(&mut self.fft_in, &mut self.fft_out, &mut self.fft_scratch)
            .expect("tamaños de FFT correctos");
        let inv_win = 1.0 / w as f32;
        for (m, c) in self.mag.iter_mut().zip(&self.fft_out) {
            *m = c.norm() * inv_win;
        }

        let s = &self.s;
        let vals = &mut self.features.values;
        let nb = self.mag.len() as f32;

        // --- Nivel global y timbre ---
        let rms = (block.iter().map(|x| x * x).sum::<f32>() / n.max(1) as f32).sqrt();
        vals[s.level] = self.rms_env.process(self.rms_gain.process(rms));
        let total: f32 = self.mag.iter().map(|m| m * m).sum::<f32>() + 1e-12;
        let weighted: f32 = self.mag.iter().zip(&self.freqs).map(|(m, f)| f * m * m).sum();
        let centroid = weighted / total;
        vals[s.brightness] = self.centroid_env.process((centroid / 8000.0).min(1.0));
        let log_mean = self.mag.iter().map(|m| (m + 1e-10).ln()).sum::<f32>() / nb;
        let mean = self.mag.iter().sum::<f32>() / nb;
        vals[s.noisiness] = self.flatness_env.process(log_mean.exp() / (mean + 1e-10));
        vals[s.centroid] = centroid;

        // --- Bandas de frecuencia ---
        for b in &mut self.bands {
            let e = (mean_sq(&self.mag[b.bins.clone()]) + 1e-20).sqrt();
            vals[b.slot] = b.env.process(b.gain.process(e));
        }

        // --- HPSS: mediana en el tiempo (armónico) y en frecuencia (percusivo) ---
        self.history[self.hist_pos].copy_from_slice(&self.mag);
        self.hist_pos = (self.hist_pos + 1) % self.kernel;
        self.hist_len = (self.hist_len + 1).min(self.kernel);
        let k = self.kernel;
        let half = k / 2;
        let last = self.mag.len() - 1;
        for i in 0..self.mag.len() {
            // percusivo: mediana sobre frecuencias vecinas (bordes repetidos)
            for j in 0..k {
                let idx = (i + j).saturating_sub(half).min(last);
                self.median_tmp[j] = self.mag[idx];
            }
            self.perc_est[i] = median(&mut self.median_tmp[..k]);
        }
        for i in 0..self.mag.len() {
            for j in 0..self.hist_len {
                self.median_tmp[j] = self.history[j][i];
            }
            let h = median(&mut self.median_tmp[..self.hist_len]);
            let p = self.perc_est[i];
            let (h2, p2) = (h * h, p * p);
            let denom = h2 + p2 + 1e-20;
            self.harm[i] = self.mag[i] * (h2 / denom);
            self.perc[i] = self.mag[i] * (p2 / denom);
        }
        vals[s.harmonic] = self
            .harm_env
            .process(self.harm_gain.process(mean_sq(&self.harm).sqrt()));
        vals[s.percussive] = self
            .perc_env
            .process(self.perc_gain.process(mean_sq(&self.perc).sqrt()));

        // flujo espectral (cuánto subió cada frecuencia desde el bloque anterior)
        let mut flux_all = 0.0;
        for i in 0..self.mag.len() {
            flux_all += (self.mag[i] - self.prev_mag[i]).max(0.0);
        }
        flux_all /= nb;
        vals[s.onset] = self.onset_all.process((1000.0 * flux_all).ln_1p());
        let any_hit = self.onset_all.triggered;

        // --- Instrumentos aproximados ---
        let mut drum_flux = 0.0;
        for inst in &mut self.insts {
            match inst.kind {
                InstrumentKind::Percussive => {
                    let mut sum = 0.0;
                    let mut min = f32::INFINITY;
                    for r in &inst.ranges {
                        let fl = mean_pos_diff(&self.perc[r.clone()], &self.prev_perc[r.clone()]);
                        let fl = (1000.0 * fl).ln_1p();
                        sum += fl;
                        min = min.min(fl);
                    }
                    // "all": todas las zonas deben golpear a la vez (ej. caja = cuerpo + chasquido)
                    let flux = if inst.combine_all {
                        min
                    } else {
                        sum / inst.ranges.len() as f32
                    };
                    drum_flux += flux;
                    vals[inst.slot] = inst.onset.as_mut().unwrap().process(flux);
                    let energy: f32 = inst.ranges.iter().map(|r| mean_sq(&self.perc[r.clone()]).sqrt()).sum();
                    vals[inst.level_slot.unwrap()] = inst.env.process(inst.gain.process(energy));
                }
                InstrumentKind::Harmonic => {
                    let energy: f32 = inst.ranges.iter().map(|r| mean_sq(&self.harm[r.clone()]).sqrt()).sum();
                    vals[inst.slot] = inst.env.process(inst.gain.process(energy));
                }
            }
        }
        std::mem::swap(&mut self.prev_mag, &mut self.mag);
        std::mem::swap(&mut self.prev_perc, &mut self.perc);

        // --- Nota de la melodía (espectro armónico, suma de armónicos) ---
        self.pitch();

        // --- Tempo y pulso ---
        if self.onset_hist.len() == self.onset_hist_cap {
            self.onset_hist.pop_front();
        }
        self.onset_hist.push_back(drum_flux);
        if self.frame.is_multiple_of(self.tempo_every) {
            self.update_tempo();
        }
        let kick_hit = self
            .kick
            .and_then(|i| self.insts[i].onset.as_ref())
            .is_some_and(|o| o.triggered);
        self.beat(kick_hit || any_hit);
        &self.features
    }

    fn pitch(&mut self) {
        let harm = &self.harm;
        let n = harm.len();
        for (c, h) in self.comp.iter_mut().zip(harm) {
            *c = h.sqrt();
        }
        self.score.copy_from_slice(&self.comp);
        for h in 2..=4usize {
            let len = n.div_ceil(h);
            for i in 0..len {
                self.score[i] += self.comp[i * h] / h as f32;
            }
        }
        let r = self.pitch_bins.clone();
        let (mut idx, mut peak) = (0usize, 0.0f32);
        for i in r.clone() {
            if self.score[i] > peak {
                peak = self.score[i];
                idx = i;
            }
        }
        let mut conf = 0.0;
        if peak > 0.0 {
            let mean = self.score[r.clone()].iter().sum::<f32>() / r.len() as f32;
            conf = 1.0 - mean / peak;
        }
        if conf >= self.pitch_conf_min && idx > 0 && idx < n - 1 {
            // interpolación parabólica sobre el espectro original
            let a = (harm[idx - 1] + 1e-12).ln();
            let b = (harm[idx] + 1e-12).ln();
            let c = (harm[idx + 1] + 1e-12).ln();
            let d = a - 2.0 * b + c;
            let shift = if d != 0.0 { 0.5 * (a - c) / d } else { 0.0 };
            self.pitch_hz = (idx as f32 + shift.clamp(-0.5, 0.5)) * self.sr / self.nfft as f32;
        }
        let s = &self.s;
        let vals = &mut self.features.values;
        vals[s.pitch_hz] = self.pitch_hz;
        let midi = if self.pitch_hz > 0.0 {
            69.0 + 12.0 * (self.pitch_hz / 440.0).log2()
        } else {
            0.0
        };
        vals[s.note] = midi;
        vals[s.pitch_class] = if self.pitch_hz > 0.0 {
            (midi.round() as i32).rem_euclid(12) as f32
        } else {
            0.0
        };
        vals[s.confidence] = conf;

        // cromagrama (qué notas suenan)
        self.chroma = [0.0; 12];
        for (h, &pc) in harm.iter().zip(&self.bin_pitch_class) {
            if pc >= 0 {
                self.chroma[pc as usize] += h * h;
            }
        }
        let max = self.chroma.iter().copied().fold(0.0f32, f32::max) + 1e-20;
        for i in 0..12 {
            vals[s.chroma[i]] = self.chroma_env[i].process(self.chroma[i] / max);
        }
    }

    fn update_tempo(&mut self) {
        let n = self.onset_hist.len();
        if (n as f32) < self.fps * 2.0 {
            return;
        }
        let mean = self.onset_hist.iter().map(|&v| v as f64).sum::<f64>() / n as f64;
        let x: Vec<f64> = self.onset_hist.iter().map(|&v| v as f64 - mean).collect();
        let std = (x.iter().map(|v| v * v).sum::<f64>() / n as f64).sqrt();
        if std < 1e-6 {
            return;
        }
        let fps = self.fps as f64;
        let lag_bpm = |lag: f64| 60.0 * fps / lag;
        // autocorrelación de la curva de golpes
        let ac: Vec<f64> = (0..n)
            .map(|lag| (0..n - lag).map(|i| x[i] * x[i + lag]).sum())
            .collect();
        let mut best: Option<(usize, f64)> = None;
        for (lag, &v) in ac.iter().enumerate().skip(1) {
            let bpm = lag_bpm(lag as f64);
            if bpm < self.tempo_min as f64 || bpm > self.tempo_max as f64 {
                continue;
            }
            // preferencia suave por tempos cercanos a 120 BPM
            let weight = (-0.5 * (bpm / 120.0).log2().powi(2)).exp();
            let score = v * weight;
            if best.is_none_or(|(_, b)| score > b) {
                best = Some((lag, score));
            }
        }
        let Some((lag, _)) = best else { return };
        // refinamiento parabólico del lag
        let lag_f = if lag >= 1 && lag + 1 < ac.len() {
            let (a, b, c) = (ac[lag - 1], ac[lag], ac[lag + 1]);
            let d = a - 2.0 * b + c;
            lag as f64 + if d != 0.0 { 0.5 * (a - c) / d } else { 0.0 }
        } else {
            lag as f64
        };
        let new_bpm = lag_bpm(lag_f) as f32;
        self.bpm = if (new_bpm - self.bpm).abs() < 8.0 {
            0.7 * self.bpm + 0.3 * new_bpm
        } else {
            new_bpm
        };
    }

    /// Oscilador de fase que se re-sincroniza suavemente con los golpes.
    fn beat(&mut self, hit: bool) {
        self.phase += self.bpm / 60.0 / self.fps;
        let mut beat = false;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
            beat = true;
        }
        if hit {
            let err = if self.phase < 0.5 { self.phase } else { self.phase - 1.0 };
            self.phase = (self.phase - 0.25 * err).rem_euclid(1.0);
        }
        self.beat_pulse = if beat { 1.0 } else { self.beat_pulse * self.beat_decay };
        let s = &self.s;
        let vals = &mut self.features.values;
        vals[s.bpm] = self.bpm;
        vals[s.phase] = self.phase;
        vals[s.beat] = self.beat_pulse;
    }
}

fn mean_sq(x: &[f32]) -> f32 {
    if x.is_empty() {
        return 0.0;
    }
    x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32
}

fn mean_pos_diff(cur: &[f32], prev: &[f32]) -> f32 {
    if cur.is_empty() {
        return 0.0;
    }
    cur.iter().zip(prev).map(|(c, p)| (c - p).max(0.0)).sum::<f32>() / cur.len() as f32
}

/// Mediana in-place (reordena `v`). Para largo par promedia los dos centrales, como numpy.
fn median(v: &mut [f32]) -> f32 {
    let n = v.len();
    if n == 0 {
        return 0.0;
    }
    let mid = n / 2;
    let (left, m, _) = v.select_nth_unstable_by(mid, f32::total_cmp);
    let m = *m;
    if n % 2 == 1 {
        m
    } else {
        let lo = left.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        0.5 * (lo + m)
    }
}
