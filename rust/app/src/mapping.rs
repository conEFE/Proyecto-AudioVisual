//! Qué parte del sonido mueve qué parte de la imagen. Todo se guarda como "preset" (JSON).

use serde::{Deserialize, Serialize};

/// Fuentes de sonido disponibles: (clave interna, nombre para mostrar, escala a 0..1).
pub const SOURCES: &[(&str, &str, f32)] = &[
    ("", "— nada —", 1.0),
    ("mix/level", "Volumen", 1.0),
    ("band/sub", "Graves profundos", 1.0),
    ("band/bass", "Graves", 1.0),
    ("band/lowmid", "Medios-graves", 1.0),
    ("band/mid", "Medios", 1.0),
    ("band/highmid", "Medios-agudos", 1.0),
    ("band/high", "Agudos", 1.0),
    ("drums/kick", "Bombo (golpe)", 1.0),
    ("drums/snare", "Caja (golpe)", 1.0),
    ("drums/hihat", "Hi-hat (golpe)", 1.0),
    ("bass", "Bajo", 1.0),
    ("lead", "Melodía / voz", 1.0),
    ("lead/pitch_class", "Nota de la melodía", 1.0 / 12.0),
    ("mix/brightness", "Brillo del sonido", 1.0),
    ("mix/noisiness", "Ruido / distorsión", 1.0),
    ("mix/percussive", "Percusión", 1.0),
    ("mix/harmonic", "Armonía", 1.0),
    ("mix/onset", "Cualquier golpe", 1.0),
    ("tempo/beat", "Pulso del tempo", 1.0),
    ("tempo/phase", "Fase del tempo", 1.0),
];

pub fn source_label(key: &str) -> &'static str {
    SOURCES.iter().find(|s| s.0 == key).map(|s| s.1).unwrap_or("?")
}

/// Parámetros visuales (los mismos para todas las escenas).
pub const PARAMS: [(&str, &str); 8] = [
    ("Tamaño", "Zoom / escala de la imagen"),
    ("Velocidad", "Qué tan rápido se mueve"),
    ("Destello", "Brillo extra (ideal para golpes)"),
    ("Color", "Tono del color (da la vuelta al círculo cromático)"),
    ("Saturación", "Intensidad del color"),
    ("Detalle", "Cantidad de detalle / repeticiones"),
    ("Distorsión", "Cuánto se deforma la imagen"),
    ("Giro", "Velocidad de rotación"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Mapping {
    /// Clave de la fuente (ver `SOURCES`); vacío = nada.
    pub source: String,
    /// Valor cuando no hay sonido.
    pub base: f32,
    /// Cuánto suma el sonido.
    pub amount: f32,
    /// Segundos que tarda en calmarse (0 = instantáneo).
    pub smooth: f32,
}

impl Mapping {
    fn new(source: &str, base: f32, amount: f32, smooth: f32) -> Self {
        Self {
            source: source.into(),
            base,
            amount,
            smooth,
        }
    }
}

pub const SCENES: [&str; 5] = ["Nebulosa", "Túnel", "Caleidoscopio", "Espectro", "Ondas"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    pub name: String,
    pub scene: usize,
    pub params: Vec<Mapping>,
    /// Cambia de escena sola cada N compases (0 = nunca).
    #[serde(default)]
    pub auto_scene_bars: u32,
}

impl Preset {
    /// Preset inicial de cada escena.
    pub fn default_for(scene: usize) -> Self {
        use Mapping as M;
        let params = match scene {
            // Tamaño, Velocidad, Destello, Color, Saturación, Detalle, Distorsión, Giro
            1 => vec![
                M::new("band/bass", 0.4, 0.5, 0.15),
                M::new("mix/level", 0.3, 0.9, 0.3),
                M::new("drums/kick", 0.1, 1.0, 0.12),
                M::new("lead/pitch_class", 0.0, 1.0, 0.6),
                M::new("lead", 0.6, 0.4, 0.3),
                M::new("drums/hihat", 0.4, 0.3, 0.1),
                M::new("drums/snare", 0.1, 0.6, 0.15),
                M::new("tempo/phase", 0.2, 0.0, 0.0),
            ],
            2 => vec![
                M::new("band/bass", 0.5, 0.4, 0.2),
                M::new("mix/level", 0.2, 0.6, 0.3),
                M::new("drums/kick", 0.1, 0.9, 0.12),
                M::new("lead/pitch_class", 0.0, 1.0, 0.6),
                M::new("lead", 0.6, 0.4, 0.3),
                M::new("drums/snare", 0.4, 0.3, 0.2),
                M::new("band/high", 0.2, 0.5, 0.2),
                M::new("bass", 0.15, 0.3, 0.4),
            ],
            3 => vec![
                M::new("drums/kick", 0.5, 0.25, 0.12),
                M::new("mix/level", 0.3, 0.5, 0.3),
                M::new("drums/kick", 0.1, 0.8, 0.12),
                M::new("lead/pitch_class", 0.0, 1.0, 0.6),
                M::new("lead", 0.7, 0.3, 0.3),
                M::new("", 0.6, 0.0, 0.0),
                M::new("drums/snare", 0.1, 0.5, 0.15),
                M::new("", 0.1, 0.0, 0.0),
            ],
            4 => vec![
                M::new("band/bass", 0.4, 0.4, 0.2),
                M::new("mix/level", 0.3, 0.7, 0.3),
                M::new("drums/kick", 0.15, 0.8, 0.12),
                M::new("lead/pitch_class", 0.0, 1.0, 0.6),
                M::new("lead", 0.6, 0.4, 0.3),
                M::new("band/mid", 0.3, 0.5, 0.2),
                M::new("bass", 0.2, 0.7, 0.2),
                M::new("", 0.0, 0.0, 0.0),
            ],
            _ => vec![
                M::new("band/bass", 0.4, 0.5, 0.2),
                M::new("mix/level", 0.2, 0.8, 0.3),
                M::new("drums/kick", 0.15, 1.0, 0.12),
                M::new("lead/pitch_class", 0.0, 1.0, 0.6),
                M::new("lead", 0.5, 0.5, 0.3),
                M::new("drums/hihat", 0.3, 0.5, 0.1),
                M::new("drums/snare", 0.2, 0.6, 0.15),
                M::new("", 0.05, 0.0, 0.0),
            ],
        };
        Self {
            name: SCENES[scene].to_string(),
            scene,
            params,
            auto_scene_bars: 0,
        }
    }
}

/// Aplica los mapeos a los valores del sonido, con suavizado.
#[derive(Default)]
pub struct Evaluator {
    smoothed: Vec<f32>,
}

impl Evaluator {
    /// `lookup(clave)` devuelve el valor del sonido. Devuelve los 8 parámetros visuales.
    pub fn eval(&mut self, preset: &Preset, dt: f32, lookup: impl Fn(&str) -> f32) -> [f32; 8] {
        self.smoothed.resize(preset.params.len(), 0.0);
        let mut out = [0.0; 8];
        for (i, m) in preset.params.iter().enumerate().take(8) {
            let scale = SOURCES.iter().find(|s| s.0 == m.source).map(|s| s.2).unwrap_or(1.0);
            let x = if m.source.is_empty() {
                0.0
            } else {
                lookup(&m.source) * scale
            };
            let s = &mut self.smoothed[i];
            // sube al instante (para que los golpes se vean), baja suave
            if x >= *s || m.smooth <= 0.0 {
                *s = x;
            } else {
                *s += (x - *s) * (1.0 - (-dt / m.smooth).exp());
            }
            out[i] = m.base + m.amount * *s;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_scene_has_all_params_with_known_sources() {
        for s in 0..SCENES.len() {
            let p = Preset::default_for(s);
            assert_eq!(p.params.len(), PARAMS.len());
            for m in &p.params {
                assert!(SOURCES.iter().any(|src| src.0 == m.source), "{}", m.source);
            }
        }
    }

    #[test]
    fn silence_gives_base_and_hits_rise_instantly_then_calm_down() {
        let p = Preset::default_for(0);
        let mut ev = Evaluator::default();
        let quiet = ev.eval(&p, 1.0 / 60.0, |_| 0.0);
        for (v, m) in quiet.iter().zip(&p.params) {
            assert!((v - m.base).abs() < 1e-6);
        }
        // golpe de bombo: el destello sube al instante
        let hit = ev.eval(&p, 1.0 / 60.0, |k| if k == "drums/kick" { 1.0 } else { 0.0 });
        assert!((hit[2] - (p.params[2].base + p.params[2].amount)).abs() < 1e-6);
        // y se calma con el tiempo
        let later = ev.eval(&p, 0.5, |_| 0.0);
        assert!(later[2] < hit[2] && later[2] > p.params[2].base);
    }

    #[test]
    fn preset_roundtrips_through_json() {
        let p = Preset::default_for(3);
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Preset>(&json).unwrap(), p);
    }

    #[test]
    fn note_source_is_scaled_to_one() {
        let mut p = Preset::default_for(0);
        p.params[3] = Mapping::new("lead/pitch_class", 0.0, 1.0, 0.0);
        let out = Evaluator::default().eval(&p, 0.016, |k| if k == "lead/pitch_class" { 9.0 } else { 0.0 });
        assert!((out[3] - 0.75).abs() < 1e-6); // La = 9/12
    }
}
