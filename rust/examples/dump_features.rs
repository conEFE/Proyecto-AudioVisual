//! Exporta todos los valores del análisis de un archivo a CSV (un renglón por bloque).
//!
//!   cargo run --release --example dump_features -- cancion.wav > valores.csv

use std::io::{BufWriter, Write};
use std::time::Instant;

use audiovisual::{Config, LiveAnalyzer, input};

fn main() -> anyhow::Result<()> {
    let path = std::env::args().nth(1).expect("uso: dump_features <archivo>");
    let data = input::decode_file(path.as_ref())?;
    let cfg = Config::builtin();
    let mut an = LiveAnalyzer::new(data.sample_rate, &cfg);
    let hop = cfg.analysis.hop_size;
    let ch = data.channels;
    let mono: Vec<f32> = data
        .samples
        .chunks_exact(ch)
        .map(|f| f.iter().sum::<f32>() / ch as f32)
        .collect();
    let mut out = BufWriter::new(std::io::stdout().lock());
    writeln!(out, "{}", an.features.names.join(","))?;
    let t0 = Instant::now();
    let mut n = 0;
    for block in mono.chunks_exact(hop) {
        let f = an.process(block);
        let row: Vec<String> = f.values.iter().map(|v| format!("{v:.6}")).collect();
        writeln!(out, "{}", row.join(","))?;
        n += 1;
    }
    eprintln!(
        "{n} bloques, {:.3} ms por bloque",
        t0.elapsed().as_secs_f64() * 1000.0 / n as f64
    );
    Ok(())
}
