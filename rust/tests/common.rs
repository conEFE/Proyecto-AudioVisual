//! Señales sintéticas compartidas por las pruebas (las mismas que tests/test_analyzer.py).
#![allow(dead_code)]

use std::f32::consts::PI;

pub const SR: u32 = 44100;

/// Generador pseudoaleatorio pequeño y determinista (xorshift + Box-Muller).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn uniform(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        ((self.0 >> 40) as f32 + 0.5) / (1u64 << 24) as f32
    }
    pub fn normal(&mut self) -> f32 {
        let (u1, u2) = (self.uniform(), self.uniform());
        (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
    }
}

fn len(seconds: f32) -> usize {
    (SR as f32 * seconds) as usize
}

/// Tono con armónicos, como un instrumento real.
pub fn tone(freq: f32, seconds: f32) -> Vec<f32> {
    (0..len(seconds))
        .map(|i| {
            let t = i as f32 / SR as f32;
            [1.0f32, 2.0, 3.0, 4.0]
                .iter()
                .map(|h| 0.3 / h * (2.0 * PI * freq * h * t).sin())
                .sum()
        })
        .collect()
}

pub fn sine(freq: f32, seconds: f32, amp: f32) -> Vec<f32> {
    (0..len(seconds))
        .map(|i| amp * (2.0 * PI * freq * i as f32 / SR as f32).sin())
        .collect()
}

pub fn kick_track(bpm: f32, seconds: f32) -> Vec<f32> {
    let mut out = vec![0.0; len(seconds)];
    let period = (SR as f32 * 60.0 / bpm) as usize;
    let n = (0.15 * SR as f32) as usize;
    let mut phase = 0.0f32;
    let kick: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR as f32;
            phase += 50.0 + 100.0 * (-t * 30.0).exp();
            0.9 * (2.0 * PI * phase / SR as f32).sin() * (-t * 20.0).exp()
        })
        .collect();
    let mut start = 0;
    while start + n < out.len() {
        for (o, k) in out[start..start + n].iter_mut().zip(&kick) {
            *o += k;
        }
        start += period;
    }
    out
}

pub fn hihat_track(bpm: f32, seconds: f32, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let mut out = vec![0.0; len(seconds)];
    let period = (SR as f32 * 60.0 / bpm) as usize;
    let n = (0.03 * SR as f32) as usize;
    let noise: Vec<f32> = (0..n).map(|_| rng.normal()).collect();
    let hit: Vec<f32> = (0..n)
        .map(|i| {
            let d = noise[i] - if i > 0 { noise[i - 1] } else { 0.0 };
            0.3 * d * (-(i as f32) / SR as f32 * 150.0).exp()
        })
        .collect();
    let mut start = period / 2;
    while start + n < out.len() {
        for (o, h) in out[start..start + n].iter_mut().zip(&hit) {
            *o += h;
        }
        start += period;
    }
    out
}

pub fn snare_track(bpm: f32, seconds: f32, seed: u64) -> Vec<f32> {
    let mut rng = Rng::new(seed);
    let mut out = vec![0.0; len(seconds)];
    let period = (SR as f32 * 60.0 / bpm) as usize;
    let n = (0.12 * SR as f32) as usize;
    let hit: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / SR as f32;
            (0.5 * (2.0 * PI * 200.0 * t).sin() + 0.4 * rng.normal()) * (-t * 35.0).exp()
        })
        .collect();
    let mut start = 0;
    while start + n < out.len() {
        for (o, h) in out[start..start + n].iter_mut().zip(&hit) {
            *o += h;
        }
        start += period;
    }
    out
}

pub fn mix(tracks: &[&[f32]]) -> Vec<f32> {
    let n = tracks.iter().map(|t| t.len()).min().unwrap_or(0);
    (0..n).map(|i| tracks.iter().map(|t| t[i]).sum()).collect()
}
