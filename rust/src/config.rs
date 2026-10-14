//! Configuración: el mismo `config.yaml` que usa la versión en Python.
//!
//! El `config.yaml` del proyecto va incluido dentro del ejecutable, así que el binario
//! funciona solo. Si hay un `config.yaml` en la carpeta actual, o se pasa `--config`,
//! sus valores se aplican encima.

use std::path::Path;

use anyhow::{Context, Result};
use indexmap::IndexMap;
use serde::Deserialize;
use serde_yaml::Value;

const DEFAULT_YAML: &str = include_str!("../../config.yaml");

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub audio: AudioCfg,
    pub osc: OscCfg,
    pub analysis: AnalysisCfg,
    pub bands: Vec<Band>,
    pub instruments: IndexMap<String, Instrument>,
    pub pitch: PitchCfg,
    pub tempo: TempoCfg,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AudioCfg {
    pub sample_rate: u32,
    #[serde(default)]
    pub device: Option<Value>,
    #[serde(default = "one")]
    pub channels: u16,
    #[serde(default)]
    pub inputs: Option<Vec<InputChannel>>,
}

fn one() -> u16 {
    1
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputChannel {
    pub channel: u16,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OscCfg {
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub prefix: String,
    #[serde(default = "sixty")]
    pub send_fps: f32,
}

fn sixty() -> f32 {
    60.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct AnalysisCfg {
    pub hop_size: usize,
    pub window_size: usize,
    pub hpss_kernel: usize,
    pub attack: f32,
    pub release: f32,
    pub autogain_seconds: f32,
    pub onset_sensitivity: f32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Band {
    pub name: String,
    pub low: f32,
    pub high: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstrumentKind {
    Percussive,
    Harmonic,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Instrument {
    #[serde(rename = "type")]
    pub kind: InstrumentKind,
    pub ranges: Vec<[f32; 2]>,
    #[serde(default = "d_sensitivity")]
    pub sensitivity: f32,
    #[serde(default = "d_refractory")]
    pub refractory: f32,
    #[serde(default = "d_pulse_release")]
    pub pulse_release: f32,
    #[serde(default = "d_min_flux")]
    pub min_flux: f32,
    #[serde(default)]
    pub combine: Option<String>,
}

fn d_sensitivity() -> f32 {
    1.5
}
fn d_refractory() -> f32 {
    0.08
}
fn d_pulse_release() -> f32 {
    0.15
}
fn d_min_flux() -> f32 {
    0.05
}

#[derive(Debug, Clone, Deserialize)]
pub struct PitchCfg {
    pub min_hz: f32,
    pub max_hz: f32,
    pub min_confidence: f32,
    #[serde(default = "d_harmonics")]
    pub harmonics: usize,
    #[serde(default = "d_bg_subtract")]
    pub background_subtract: f32,
    #[serde(default = "d_bg_seconds")]
    pub background_seconds: f32,
}

fn d_harmonics() -> usize {
    8
}
fn d_bg_subtract() -> f32 {
    0.9
}
fn d_bg_seconds() -> f32 {
    1.0
}

#[derive(Debug, Clone, Deserialize)]
pub struct TempoCfg {
    pub min_bpm: f32,
    pub max_bpm: f32,
    pub initial_bpm: f32,
    pub history_seconds: f32,
    pub update_seconds: f32,
}

fn merge(base: &mut Value, over: Value) {
    match (base, over) {
        (Value::Mapping(b), Value::Mapping(o)) => {
            for (k, v) in o {
                match b.get_mut(&k) {
                    Some(slot) if slot.is_mapping() && v.is_mapping() => merge(slot, v),
                    _ => {
                        b.insert(k, v);
                    }
                }
            }
        }
        (b, o) => *b = o,
    }
}

impl Config {
    /// Configuración incluida en el binario, sin leer archivos.
    pub fn builtin() -> Self {
        serde_yaml::from_str(DEFAULT_YAML).expect("config.yaml incluido es válido")
    }

    /// Configuración incluida + `./config.yaml` (si existe) + `extra` (si se indica).
    pub fn load(extra: Option<&Path>) -> Result<Self> {
        let mut value: Value = serde_yaml::from_str(DEFAULT_YAML)?;
        let local = Path::new("config.yaml");
        let mut layers: Vec<&Path> = Vec::new();
        if local.exists() {
            layers.push(local);
        }
        if let Some(p) = extra {
            layers.push(p);
        }
        for path in layers {
            let text = std::fs::read_to_string(path).with_context(|| format!("no se pudo leer {}", path.display()))?;
            let over: Value =
                serde_yaml::from_str(&text).with_context(|| format!("YAML inválido en {}", path.display()))?;
            if !over.is_null() {
                merge(&mut value, over);
            }
        }
        Ok(serde_yaml::from_value(value)?)
    }
}
