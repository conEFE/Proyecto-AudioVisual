//! Bloques DSP pequeños y reutilizables: suavizado, auto-ganancia y detección de golpes.

use std::collections::VecDeque;

/// Convierte un tiempo de respuesta (s) en coeficiente de suavizado por frame.
pub fn time_to_coef(seconds: f32, fps: f32) -> f32 {
    if seconds <= 0.0 {
        0.0
    } else {
        (-1.0 / (seconds * fps)).exp()
    }
}

/// Seguidor de envolvente con ataque y caída independientes.
#[derive(Debug, Clone)]
pub struct Envelope {
    attack: f32,
    release: f32,
    pub value: f32,
}

impl Envelope {
    pub fn new(attack: f32, release: f32, fps: f32) -> Self {
        Self {
            attack: time_to_coef(attack, fps),
            release: time_to_coef(release, fps),
            value: 0.0,
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let coef = if x > self.value { self.attack } else { self.release };
        self.value = coef * self.value + (1.0 - coef) * x;
        self.value
    }
}

/// Normaliza una señal a 0..1 siguiendo su pico reciente.
///
/// Así los valores enviados a TouchDesigner tienen siempre un rango útil,
/// sin importar el volumen de entrada.
#[derive(Debug, Clone)]
pub struct AutoGain {
    decay: f32,
    floor: f32,
    peak: f32,
}

impl AutoGain {
    pub fn new(decay_seconds: f32, fps: f32) -> Self {
        let floor = 1e-6;
        Self {
            decay: time_to_coef(decay_seconds, fps),
            floor,
            peak: floor,
        }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        self.peak = x.max(self.peak * self.decay).max(self.floor);
        (x / self.peak).min(1.0)
    }
}

/// Detecta golpes en una curva de flujo espectral con umbral adaptativo.
///
/// Devuelve un pulso que salta a 1 en el golpe y cae suavemente, para que el
/// receptor no se pierda el evento aunque lea a otra tasa de frames.
#[derive(Debug, Clone)]
pub struct OnsetDetector {
    history: VecDeque<f32>,
    capacity: usize,
    sensitivity: f32,
    refractory: u32,
    cooldown: u32,
    min_flux: f32,
    pulse_decay: f32,
    pub pulse: f32,
    pub triggered: bool,
}

impl OnsetDetector {
    pub fn new(fps: f32, sensitivity: f32, refractory_seconds: f32, pulse_release: f32, min_flux: f32) -> Self {
        let capacity = 4.max(fps as usize); // ~1 s de historia
        Self {
            history: VecDeque::with_capacity(capacity),
            capacity,
            sensitivity,
            refractory: 1.max((refractory_seconds * fps) as u32),
            cooldown: 0,
            min_flux,
            pulse_decay: time_to_coef(pulse_release, fps),
            pulse: 0.0,
            triggered: false,
        }
    }

    pub fn process(&mut self, flux: f32) -> f32 {
        self.triggered = false;
        if self.history.len() >= 4 {
            let n = self.history.len() as f32;
            let mean = self.history.iter().sum::<f32>() / n;
            let var = self.history.iter().map(|h| (h - mean).powi(2)).sum::<f32>() / n;
            let threshold = mean + self.sensitivity * var.sqrt();
            if self.cooldown == 0 && flux > threshold.max(self.min_flux) {
                self.triggered = true;
                self.cooldown = self.refractory;
            }
        }
        if self.history.len() == self.capacity {
            self.history.pop_front();
        }
        self.history.push_back(flux);
        self.cooldown = self.cooldown.saturating_sub(1);
        self.pulse = if self.triggered {
            1.0
        } else {
            self.pulse * self.pulse_decay
        };
        self.pulse
    }
}
