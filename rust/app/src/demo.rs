//! Canción de demostración generada por código (para probar sin micrófono ni archivos).
//! 120 BPM: bajo, melodía y batería (bombo, caja, hi-hat). La batería entra a los 4 s.

use std::f32::consts::PI;

use audiovisual::input::AudioData;

const SR: u32 = 44100;

pub fn song(seconds: f32) -> AudioData {
    let n = (SR as f32 * seconds) as usize;
    let beat = 0.5f32; // 120 BPM
    let hz = |m: f32| 440.0 * 2f32.powf((m - 69.0) / 12.0);
    let roots = [45.0, 41.0, 48.0, 43.0];
    let melody = [
        69.0, 72.0, 76.0, 74.0, 72.0, 69.0, 67.0, 64.0, 65.0, 69.0, 72.0, 77.0, 76.0, 74.0, 72.0, 71.0,
    ];
    let mut out = vec![0.0f32; n];
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let mut noise = move || {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    };
    for (i, o) in out.iter_mut().enumerate() {
        let t = i as f32 / SR as f32;
        // bajo: una nota por compás
        let bar = (t / (4.0 * beat)) as usize;
        let tb = t - bar as f32 * 4.0 * beat;
        let f = hz(roots[bar % 4]);
        let mut x = 0.3 * ((2.0 * PI * f * tb).sin() + 0.4 * (4.0 * PI * f * tb).sin()) * (tb * 20.0).min(1.0);
        // melodía: una nota por tiempo
        let b = (t / beat) as usize;
        let tm = t - b as f32 * beat;
        let fm = hz(melody[b % 16]);
        let env = (tm * 30.0).min(1.0) * (-tm * 2.5).exp();
        x += 0.22
            * (1..=3)
                .map(|h| (2.0 * PI * fm * h as f32 * tm).sin() / h as f32)
                .sum::<f32>()
            * env;
        // batería desde los 4 s
        if t >= 4.0 {
            // bombo en cada tiempo
            let tk = tm;
            let phase = 50.0 * tk + 100.0 / 30.0 * (1.0 - (-30.0 * tk).exp());
            x += 0.8 * (2.0 * PI * phase).sin() * (-tk * 20.0).exp();
            // hi-hat en cada medio tiempo (a contratiempo)
            let th = (t + beat / 4.0) % (beat / 2.0);
            if th < 0.03 {
                x += 0.15 * noise() * (-th * 150.0).exp();
            }
            // caja en los tiempos 2 y 4
            if b % 2 == 1 {
                x += (0.35 * (2.0 * PI * 200.0 * tm).sin() + 0.3 * noise()) * (-tm * 35.0).exp();
            }
        }
        *o = x;
    }
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())) * 1.1;
    for v in &mut out {
        *v /= peak;
    }
    AudioData {
        samples: out,
        channels: 1,
        sample_rate: SR,
    }
}
