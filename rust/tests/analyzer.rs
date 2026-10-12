//! Las mismas pruebas que `tests/test_analyzer.py`, sobre la versión en Rust.

mod common;

use std::collections::HashMap;
use std::time::Instant;

use audiovisual::{Config, LiveAnalyzer};
use common::*;

type Frame = HashMap<String, f32>;

fn run(signal: &[f32]) -> (LiveAnalyzer, Vec<Frame>) {
    let cfg = Config::builtin();
    let mut an = LiveAnalyzer::new(SR, &cfg);
    let hop = cfg.analysis.hop_size;
    let mut frames = Vec::new();
    let mut i = 0;
    while i + hop <= signal.len() {
        let f = an.process(&signal[i..i + hop]);
        frames.push(f.iter().map(|(k, v)| (k.to_string(), v)).collect());
        i += hop;
    }
    (an, frames)
}

fn count(frames: &[Frame], key: &str) -> usize {
    frames.iter().filter(|f| f[key] == 1.0).count()
}

fn median(mut v: Vec<f32>) -> f32 {
    v.sort_by(f32::total_cmp);
    v[v.len() / 2]
}

#[test]
fn pitch_a440() {
    let (_, frames) = run(&tone(440.0, 1.5));
    let hz = median(frames[frames.len() - 40..].iter().map(|f| f["lead/pitch_hz"]).collect());
    assert!((hz - 440.0).abs() < 8.0, "{hz}");
    let last = frames.last().unwrap();
    assert_eq!(last["lead/pitch_class"], 9.0); // La
    assert!(last["chroma/a"] > 0.9);
}

#[test]
fn pure_sine_pitch() {
    let (_, frames) = run(&sine(220.0, 1.5, 0.3));
    let hz = median(frames[frames.len() - 20..].iter().map(|f| f["lead/pitch_hz"]).collect());
    assert!((hz - 220.0).abs() < 8.0, "{hz}");
}

#[test]
fn bass_vs_lead() {
    let (_, low) = run(&tone(80.0, 1.0));
    let (_, high) = run(&tone(880.0, 1.0));
    let (low, high) = (low.last().unwrap(), high.last().unwrap());
    assert!(low["band/bass"] > low["band/high"]);
    assert!(high["band/mid"] > 0.5);
    assert!(low["band/bass"] > 0.5);
}

#[test]
fn kick_onsets_and_tempo() {
    let (an, frames) = run(&kick_track(120.0, 10.0));
    let hits = count(&frames, "drums/kick");
    assert!((17..=21).contains(&hits), "{hits}"); // 20 bombos en 10 s
    let bpm = frames.last().unwrap()["tempo/bpm"];
    assert!((bpm - 120.0).abs() < 4.0, "{bpm}");
    assert!(frames.last().unwrap()["mix/percussive"] > 0.0);
    assert!(count(&frames, "drums/snare") <= 2);
    assert!(count(&frames, "drums/hihat") <= 2);
    assert!(an.fps > 80.0);
}

#[test]
fn hihat_detected_not_kick() {
    let (_, frames) = run(&hihat_track(120.0, 4.0, 0));
    assert!(count(&frames, "drums/hihat") >= 6);
    assert!(count(&frames, "drums/kick") <= 1);
}

#[test]
fn snare_detected() {
    let (_, frames) = run(&snare_track(120.0, 4.0, 1));
    assert!(count(&frames, "drums/snare") >= 6);
}

#[test]
fn silence_is_stable() {
    let (_, frames) = run(&vec![0.0; SR as usize]);
    let f = frames.last().unwrap();
    for (k, v) in f {
        assert!(v.is_finite(), "{k}");
    }
    assert_eq!(f["mix/level"], 0.0);
    assert_eq!(f["drums/kick"], 0.0);
}

#[test]
fn noise_has_no_pitch_confidence() {
    let mut rng = Rng::new(7);
    let noise: Vec<f32> = (0..SR as usize * 2).map(|_| 0.2 * rng.normal()).collect();
    let (_, frames) = run(&noise);
    let conf = frames[frames.len() - 50..]
        .iter()
        .map(|f| f["lead/confidence"])
        .fold(0.0, f32::max);
    assert!(conf < 0.6, "{conf}");
}

#[test]
fn values_normalized() {
    let sig = mix(&[&kick_track(100.0, 3.0), &tone(220.0, 3.0), &hihat_track(100.0, 3.0, 0)]);
    let (_, frames) = run(&sig);
    let unbounded = [
        "tempo/bpm",
        "lead/pitch_hz",
        "lead/note",
        "lead/pitch_class",
        "mix/centroid_hz",
    ];
    for f in &frames {
        for (k, &v) in f {
            if !unbounded.contains(&k.as_str()) {
                assert!((-1e-6..=1.0 + 1e-6).contains(&v), "{k} = {v}");
            }
        }
    }
}

#[test]
fn realtime_budget() {
    let cfg = Config::builtin();
    let mut an = LiveAnalyzer::new(SR, &cfg);
    let sig = mix(&[&kick_track(120.0, 2.0), &tone(330.0, 2.0)]);
    let hop = cfg.analysis.hop_size;
    let t0 = Instant::now();
    let mut n = 0;
    for chunk in sig.chunks_exact(hop) {
        an.process(chunk);
        n += 1;
    }
    let per_frame = t0.elapsed().as_secs_f64() / n as f64;
    assert!(per_frame < hop as f64 / SR as f64, "{per_frame}"); // más rápido que tiempo real
}
