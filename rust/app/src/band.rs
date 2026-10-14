//! Modo "Banda": cada instrumento tiene su propia capa visual, todas a la vez.
//! Este módulo guarda la configuración (preset) y convierte el sonido en lo que dibuja el shader.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// Una capa por instrumento: (nombre, ayuda, estilos).
pub const LAYERS: [(&str, &str, [&str; 3]); 6] = [
    ("Bombo", "Golpes graves de la batería", ["Anillos", "Bloque", "Mira"]),
    ("Caja", "Golpes secos de la batería", ["Cortes", "Rayos", "Píxeles"]),
    ("Hi-hat", "Platillos y golpes agudos", ["Chispas", "Estática", "Glifos"]),
    ("Bajo", "Notas graves sostenidas", ["Onda", "Barras", "Vúmetro"]),
    (
        "Melodía",
        "Voz, guitarra, sintes: dibuja la forma de la melodía",
        ["Línea", "Escalera", "Puntos"],
    ),
    (
        "Armonía",
        "Las 12 notas: qué acorde suena",
        ["Teclas", "Orbes", "Acorde"],
    ),
];

/// Las tres primeras capas son golpes (la sensibilidad cambia el umbral de detección);
/// en las otras la sensibilidad es una ganancia.
pub const DRUM_LAYERS: usize = 3;

/// Paletas: un color por capa.
pub const PALETTES: [(&str, [[f32; 3]; 6]); 4] = [
    (
        "Neón",
        [
            [1.0, 0.1, 0.45],
            [1.0, 1.0, 1.0],
            [0.2, 1.0, 1.0],
            [0.55, 0.3, 1.0],
            [0.75, 1.0, 0.1],
            [1.0, 0.75, 0.1],
        ],
    ),
    ("Monocromo", [[1.0, 1.0, 1.0]; 6]),
    (
        "Error",
        [
            [1.0, 0.05, 0.05],
            [1.0, 1.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.6, 0.0, 0.0],
            [1.0, 0.25, 0.25],
            [0.85, 0.85, 0.85],
        ],
    ),
    (
        "Ácido",
        [
            [0.7, 1.0, 0.0],
            [1.0, 0.0, 1.0],
            [0.0, 1.0, 0.6],
            [0.0, 0.4, 1.0],
            [1.0, 1.0, 0.0],
            [1.0, 0.4, 0.0],
        ],
    ),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Layer {
    pub on: bool,
    pub style: usize,
    pub color: [f32; 3],
    /// 0..2: brillo de la capa.
    pub intensity: f32,
    /// 0..2: tamaño / grosor.
    pub size: f32,
    /// 0..1 (0.5 = calibrado). Golpes: detectar más suaves. Otras capas: ganancia.
    #[serde(default = "half")]
    pub sensitivity: f32,
}

fn half() -> f32 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Glitch {
    /// Glitch permanente (0..1).
    pub base: f32,
    /// Cuánto glitch suman los golpes de caja y bombo (0..1).
    pub hits: f32,
    /// Caos: pixelado, inversiones, más bloques (0..1).
    pub chaos: f32,
    /// Líneas de TV (0..1).
    pub scanlines: f32,
    pub grid: bool,
    pub legend: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    pub name: String,
    pub layers: Vec<Layer>,
    pub glitch: Glitch,
}

impl Default for Preset {
    fn default() -> Self {
        let colors = PALETTES[0].1;
        let layers = (0..LAYERS.len())
            .map(|i| Layer {
                on: true,
                style: 0,
                color: colors[i],
                intensity: 1.0,
                size: 1.0,
                sensitivity: 0.5,
            })
            .collect();
        Self {
            name: "Banda".into(),
            layers,
            glitch: Glitch {
                base: 0.05,
                hits: 0.6,
                chaos: 0.4,
                scanlines: 0.35,
                grid: true,
                legend: true,
            },
        }
    }
}

impl Preset {
    pub fn apply_palette(&mut self, palette: usize) {
        for (layer, color) in self.layers.iter_mut().zip(PALETTES[palette].1) {
            layer.color = color;
        }
    }

    /// Repara presets guardados con menos capas (versiones futuras pueden sumar capas).
    pub fn normalized(mut self) -> Self {
        let def = Preset::default();
        while self.layers.len() < def.layers.len() {
            self.layers.push(def.layers[self.layers.len()].clone());
        }
        for l in &mut self.layers {
            l.style = l.style.min(2);
            l.sensitivity = l.sensitivity.clamp(0.0, 1.0);
        }
        self
    }
}

pub const HISTORY: usize = 48;

/// Lo que recibe el shader en cada cuadro.
#[derive(Clone, Debug)]
pub struct BandUniforms {
    pub time: f32,
    pub seed: f32,
    pub on: [f32; 6],
    pub style: [f32; 6],
    pub color: [[f32; 3]; 6],
    pub intensity: [f32; 6],
    pub size: [f32; 6],
    pub kick_age: [f32; 4],
    pub snare_age: f32,
    pub snare_seed: f32,
    pub hat_age: f32,
    pub hat_seed: f32,
    pub hat_level: f32,
    pub bass: f32,
    pub bands: [f32; 6],
    pub lead: f32,
    pub lead_y: [f32; HISTORY],
    pub lead_a: [f32; HISTORY],
    pub chroma: [f32; 12],
    pub glitch: f32,
    pub chaos: f32,
    pub scanlines: f32,
    pub grid: f32,
}

/// Actividad de cada capa (0..1), para la leyenda y los medidores del panel.
pub type Activity = [f32; 6];

const NOTES: [&str; 12] = ["c", "cs", "d", "ds", "e", "f", "fs", "g", "gs", "a", "as", "b"];
const BAND_KEYS: [&str; 6] = [
    "band/sub",
    "band/bass",
    "band/lowmid",
    "band/mid",
    "band/highmid",
    "band/high",
];

/// Estado que se acumula entre cuadros: edad de los golpes, historia de la melodía, semillas.
pub struct BandState {
    time: f32,
    kick_age: [f32; 4],
    snare_age: f32,
    snare_hits: u32,
    hat_age: f32,
    hat_hits: u32,
    lead_y: VecDeque<f32>,
    lead_a: VecDeque<f32>,
    last_y: f32,
    sample_timer: f32,
    hit_env: f32,
}

impl Default for BandState {
    fn default() -> Self {
        Self {
            time: 0.0,
            kick_age: [99.0; 4],
            snare_age: 99.0,
            snare_hits: 0,
            hat_age: 99.0,
            hat_hits: 0,
            lead_y: VecDeque::from(vec![0.5; HISTORY]),
            lead_a: VecDeque::from(vec![0.0; HISTORY]),
            last_y: 0.5,
            sample_timer: 0.0,
            hit_env: 0.0,
        }
    }
}

impl BandState {
    /// `get(clave)` devuelve el valor del análisis de audio.
    pub fn update(&mut self, preset: &Preset, dt: f32, get: impl Fn(&str) -> f32) -> (BandUniforms, Activity) {
        self.time += dt;
        let hit = |k: &str| get(k) >= 0.999;

        for a in &mut self.kick_age {
            *a += dt;
        }
        if hit("drums/kick") {
            self.kick_age.rotate_right(1);
            self.kick_age[0] = 0.0;
        }
        self.snare_age += dt;
        if hit("drums/snare") {
            self.snare_age = 0.0;
            self.snare_hits += 1;
        }
        self.hat_age += dt;
        if hit("drums/hihat") {
            self.hat_age = 0.0;
            self.hat_hits += 1;
        }

        let gain = |i: usize| 2f32.powf((preset.layers.get(i).map_or(0.5, |l| l.sensitivity) - 0.5) * 3.0);
        // historia de la melodía: ~1.6 s a la vista, de izquierda (antes) a derecha (ahora)
        let lead = (get("lead") * gain(4)).min(1.0);
        let audible = get("lead/confidence") > 0.4 && lead > 0.08;
        if audible {
            let note = get("lead/note");
            self.last_y = ((note - 40.0) / 48.0).clamp(0.0, 1.0);
        }
        self.sample_timer += dt;
        while self.sample_timer >= 1.0 / 30.0 {
            self.sample_timer -= 1.0 / 30.0;
            self.lead_y.pop_front();
            self.lead_y.push_back(self.last_y);
            self.lead_a.pop_front();
            self.lead_a.push_back(if audible { lead } else { 0.0 });
        }

        // glitch: base + golpes (sube al instante, baja rápido)
        let g = &preset.glitch;
        let pulse = get("drums/snare").max(get("drums/kick") * 0.6);
        self.hit_env = (self.hit_env * (-dt * 6.0).exp()).max(pulse);
        let glitch = (g.base + g.hits * self.hit_env).clamp(0.0, 1.0);
        // la semilla cambia con cada golpe y, si hay glitch, varias veces por segundo
        let jitter = if glitch > 0.05 {
            (self.time * (4.0 + 20.0 * g.chaos)).floor()
        } else {
            0.0
        };
        let seed = (self.snare_hits * 7 + self.hat_hits) as f32 * 0.37 + jitter * 0.113;

        let mut u = BandUniforms {
            time: self.time,
            seed: seed % 1000.0,
            on: [0.0; 6],
            style: [0.0; 6],
            color: [[0.0; 3]; 6],
            intensity: [0.0; 6],
            size: [0.0; 6],
            kick_age: self.kick_age,
            snare_age: self.snare_age,
            snare_seed: (self.snare_hits as f32 * 1.618) % 100.0,
            hat_age: self.hat_age,
            hat_seed: (self.hat_hits as f32 * 2.414) % 100.0,
            hat_level: get("drums/hihat_level"),
            bass: (get("bass") * gain(3)).min(1.0),
            bands: BAND_KEYS.map(&get),
            lead,
            lead_y: [0.0; HISTORY],
            lead_a: [0.0; HISTORY],
            // sensibilidad de armonía: más alta = se encienden notas más débiles
            chroma: NOTES.map(|n| get(&format!("chroma/{n}")).max(0.0).powf(1.0 / gain(5))),
            glitch,
            chaos: g.chaos,
            scanlines: g.scanlines,
            grid: if g.grid { 1.0 } else { 0.0 },
        };
        for (i, l) in preset.layers.iter().enumerate().take(6) {
            u.on[i] = if l.on { 1.0 } else { 0.0 };
            u.style[i] = l.style as f32;
            u.color[i] = l.color;
            u.intensity[i] = l.intensity;
            u.size[i] = l.size;
        }
        for (i, (y, a)) in self.lead_y.iter().zip(&self.lead_a).enumerate() {
            u.lead_y[i] = *y;
            u.lead_a[i] = *a;
        }

        let fade = |age: f32, k: f32| (-age * k).exp();
        let activity = [
            fade(self.kick_age[0], 6.0),
            fade(self.snare_age, 6.0),
            fade(self.hat_age, 10.0),
            u.bass,
            if audible { lead } else { 0.0 },
            u.chroma.iter().copied().fold(0.0, f32::max) * get("mix/harmonic"),
        ];
        (u, activity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preset_has_one_layer_per_instrument() {
        let p = Preset::default();
        assert_eq!(p.layers.len(), LAYERS.len());
        assert!(p.layers.iter().all(|l| l.on));
    }

    #[test]
    fn kick_hit_starts_a_new_ring() {
        let p = Preset::default();
        let mut s = BandState::default();
        let (u, _) = s.update(&p, 0.016, |_| 0.0);
        assert!(u.kick_age[0] > 10.0);
        let (u, act) = s.update(&p, 0.016, |k| if k == "drums/kick" { 1.0 } else { 0.0 });
        assert_eq!(u.kick_age[0], 0.0);
        assert!(act[0] > 0.99); // bombo
        // el siguiente golpe empuja el anillo anterior
        let (u, _) = s.update(&p, 0.1, |k| if k == "drums/kick" { 1.0 } else { 0.0 });
        assert_eq!(u.kick_age[0], 0.0);
        assert!((u.kick_age[1] - 0.1).abs() < 1e-6);
    }

    #[test]
    fn snare_raises_glitch_and_changes_seed() {
        let p = Preset::default();
        let mut s = BandState::default();
        let (quiet, _) = s.update(&p, 0.016, |_| 0.0);
        let (hit, _) = s.update(&p, 0.016, |k| if k == "drums/snare" { 1.0 } else { 0.0 });
        assert!(hit.glitch > quiet.glitch + 0.3);
        assert_ne!(hit.snare_seed, quiet.snare_seed);
    }

    #[test]
    fn melody_history_follows_the_note() {
        let p = Preset::default();
        let mut s = BandState::default();
        let get = |k: &str| match k {
            "lead" => 0.8,
            "lead/confidence" => 0.9,
            "lead/note" => 64.0, // Mi
            _ => 0.0,
        };
        let mut u = s.update(&p, 0.0, get).0;
        for _ in 0..10 {
            u = s.update(&p, 1.0 / 30.0, get).0;
        }
        assert!((u.lead_y[HISTORY - 1] - 0.5).abs() < 1e-6); // (64-40)/48
        assert!(u.lead_a[HISTORY - 1] > 0.7);
        assert_eq!(u.lead_a[0], 0.0); // lo viejo sigue en silencio
    }

    #[test]
    fn preset_roundtrips_and_palettes_apply() {
        let mut p = Preset::default();
        p.apply_palette(1);
        assert!(p.layers.iter().all(|l| l.color == [1.0, 1.0, 1.0]));
        let json = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Preset>(&json).unwrap(), p);
    }
}
