//! Detector de golpes de batería (bombo, caja, hi-hat) calibrado con datos.
//! Mismo cálculo que `visualizer/drums.py`; los pesos vienen de `drum_model.json`
//! (generado por `tools/bench/train_drums.py`), incluido dentro del ejecutable.
//!
//! Por cada bloque mide, en 15 zonas de frecuencia, cuánto subió el sonido (flujo) y cuánto se
//! sostiene, y combina esos números con los pesos aprendidos. La caja y el bombo esperan unos
//! cuadros antes de decidir: así distinguen "caja + hi-hat" de "bombo + hi-hat".

use std::collections::VecDeque;

use indexmap::IndexMap;
use serde::Deserialize;

const MODEL_JSON: &str = include_str!("../../drum_model.json");

#[derive(Debug, Clone, Deserialize)]
pub struct DrumModel {
    pub edges: Vec<f32>,
    pub gain: f64,
    pub level_seconds: f64,
    pub rel_seconds: f64,
    pub rel_eps: f64,
    pub rel_init: f64,
    pub refractory: f64,
    pub models: IndexMap<String, ModelSpec>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelSpec {
    pub context: usize,
    pub features: Vec<String>,
    pub weights: Vec<f64>,
    pub bias: f64,
    pub threshold: f64,
    #[serde(default)]
    pub refractory: Option<f64>,
}

impl DrumModel {
    pub fn builtin() -> Self {
        serde_json::from_str(MODEL_JSON).expect("drum_model.json incluido es válido")
    }
}

struct Detector {
    name: String,
    k: usize,
    flux: bool,
    rel: bool,
    lvl: bool,
    weights: Vec<f64>,
    bias: f64,
    threshold: f64,
    offset: f64,
    rows: VecDeque<Vec<f64>>,
    prev: f64,
    cool: u32,
    cooldown: u32,
    triggered: bool,
    score: f64,
}

pub struct DrumDetector {
    bands: Vec<std::ops::Range<usize>>,
    gain: f64,
    level_decay: f64,
    peak: f64,
    rel_a: f64,
    rel_eps: f64,
    rel_mean: Vec<f64>,
    prev: Vec<f64>,
    flux: Vec<f64>,
    rel: Vec<f64>,
    levels: VecDeque<Vec<f64>>,
    levels_cap: usize,
    dets: Vec<Detector>,
}

impl DrumDetector {
    /// `freqs`: frecuencia de cada bin del espectro; `fps`: bloques por segundo.
    pub fn new(model: &DrumModel, freqs: &[f32], fps: f32) -> Self {
        let fps = fps as f64;
        let bands: Vec<_> = model
            .edges
            .windows(2)
            .map(|e| {
                let a = freqs.partition_point(|&f| f < e[0]);
                let b = freqs.partition_point(|&f| f < e[1]);
                a..b.max(a)
            })
            .collect();
        let n = 2 * bands.len();
        let mut max_k = 0;
        let dets = model
            .models
            .iter()
            .map(|(name, m)| {
                max_k = max_k.max(m.context);
                let has = |f: &str| m.features.iter().any(|x| x == f);
                let row_len = n * (has("flux") as usize + has("rel") as usize + has("lvl") as usize);
                assert_eq!(
                    m.weights.len(),
                    row_len * (m.context + 1),
                    "pesos de {name} con tamaño incorrecto"
                );
                Detector {
                    name: name.clone(),
                    k: m.context,
                    flux: has("flux"),
                    rel: has("rel"),
                    lvl: has("lvl"),
                    weights: m.weights.clone(),
                    bias: m.bias,
                    threshold: m.threshold,
                    offset: 0.0,
                    rows: VecDeque::from(vec![vec![0.0; row_len]; m.context + 1]),
                    prev: 0.0,
                    cool: 0,
                    cooldown: (m.refractory.unwrap_or(model.refractory) * fps) as u32,
                    triggered: false,
                    score: 0.0,
                }
            })
            .collect();
        Self {
            bands,
            gain: model.gain,
            level_decay: (-1.0 / (model.level_seconds * fps)).exp(),
            peak: 1e-4,
            rel_a: (-1.0 / (model.rel_seconds * fps)).exp(),
            rel_eps: model.rel_eps,
            rel_mean: vec![model.rel_init; n],
            prev: vec![0.0; n],
            flux: vec![0.0; n],
            rel: vec![0.0; n],
            levels: VecDeque::with_capacity(max_k + 2),
            levels_cap: max_k + 2,
            dets,
        }
    }

    /// Nombres de los instrumentos que detecta ("kick", "snare", "hihat").
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.dets.iter().map(|d| d.name.as_str())
    }

    /// Sensibilidad 0..1 (0.5 = calibrado). Más alto = detecta golpes más suaves.
    pub fn set_sensitivity(&mut self, name: &str, s: f32) {
        if let Some(d) = self.dets.iter_mut().find(|d| d.name == name) {
            d.offset = (s as f64 - 0.5) * 6.0;
        }
    }

    pub fn triggered(&self, name: &str) -> bool {
        self.dets.iter().any(|d| d.name == name && d.triggered)
    }

    pub fn score(&self, name: &str) -> f64 {
        self.dets.iter().find(|d| d.name == name).map_or(0.0, |d| d.score)
    }

    /// Procesa un bloque: `perc` = parte percusiva del espectro, `mag` = espectro completo.
    pub fn process(&mut self, perc: &[f32], mag: &[f32]) {
        let mean = mag.iter().map(|&v| v as f64).sum::<f64>() / mag.len() as f64;
        self.peak = mean.max(self.peak * self.level_decay).max(1e-6);
        let scale = self.gain / self.peak;
        let nb = self.bands.len();
        // reutiliza el vector más viejo de la historia (sin reservar memoria por bloque)
        let mut lv = if self.levels.len() >= self.levels_cap {
            self.levels.pop_front().expect("historia no vacía")
        } else {
            vec![0.0; 2 * nb]
        };
        for (i, r) in self.bands.iter().enumerate() {
            let (mut p, mut m) = (0.0, 0.0);
            if !r.is_empty() {
                let len = r.len() as f64;
                p = perc[r.clone()].iter().map(|&v| v as f64).sum::<f64>() / len;
                m = mag[r.clone()].iter().map(|&v| v as f64).sum::<f64>() / len;
            }
            lv[i] = (scale * p).ln_1p();
            lv[nb + i] = (scale * m).ln_1p();
        }
        for (i, &l) in lv.iter().enumerate() {
            let flux = (l - self.prev[i]).max(0.0);
            self.flux[i] = flux;
            self.rel[i] = flux / (self.rel_mean[i] + self.rel_eps);
            self.rel_mean[i] = self.rel_a * self.rel_mean[i] + (1.0 - self.rel_a) * flux;
        }
        self.prev.copy_from_slice(&lv);
        if self.levels.is_empty() {
            // primer bloque: la historia arranca llena con este nivel
            for _ in 1..self.levels_cap {
                self.levels.push_back(lv.clone());
            }
        }
        self.levels.push_back(lv);
        let levels = &self.levels;
        let last = levels.len() - 1;
        for d in &mut self.dets {
            // fila de características de este bloque
            let mut row = d.rows.pop_front().expect("historia con k+1 filas");
            row.clear();
            if d.flux {
                row.extend_from_slice(&self.flux);
            }
            if d.rel {
                row.extend_from_slice(&self.rel);
            }
            if d.lvl {
                let now = &levels[last];
                let before = &levels[last - (d.k + 1)];
                row.extend(now.iter().zip(before).map(|(a, b)| a - b));
            }
            d.rows.push_back(row);
            // contexto: la fila más nueva primero
            let mut score = d.bias;
            let mut w = d.weights.iter();
            for r in d.rows.iter().rev() {
                for (x, wi) in r.iter().zip(&mut w) {
                    score += x * wi;
                }
            }
            d.cool = d.cool.saturating_sub(1);
            d.triggered = score > d.threshold - d.offset && d.cool == 0 && score >= d.prev;
            if d.triggered {
                d.cool = d.cooldown;
            }
            d.prev = score;
            d.score = score;
        }
    }
}
