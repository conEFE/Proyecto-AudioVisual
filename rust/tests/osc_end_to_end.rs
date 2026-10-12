//! Archivo WAV → análisis → OSC → receptor UDP local.

mod common;

use std::collections::HashMap;
use std::net::UdpSocket;
use std::time::Duration;

use audiovisual::{Config, LiveAnalyzer, OscSender, input};
use common::*;

#[test]
fn file_to_osc() {
    let dir = std::env::temp_dir().join(format!("av-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("test.wav");
    let mut sig = sine(220.0, 2.0, 0.3);
    for i in (0..sig.len()).step_by(SR as usize / 2) {
        sig[i] += 0.9; // clics
    }
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SR,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&path, spec).unwrap();
    for s in &sig {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        w.write_sample(v).unwrap();
        w.write_sample(v).unwrap();
    }
    w.finalize().unwrap();

    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver.set_read_timeout(Some(Duration::from_millis(300))).unwrap();
    let port = receiver.local_addr().unwrap().port();

    let cfg = Config::builtin();
    let mut src = input::file_input(&path, false, false, false).unwrap();
    assert_eq!(src.channels, 2);
    assert_eq!(src.sample_rate, SR);
    let mut an = LiveAnalyzer::new(src.sample_rate, &cfg);
    let mut sender = OscSender::new("127.0.0.1", port, "av", 0.0).unwrap();
    let g = sender.register("", &an.features);
    let hop = cfg.analysis.hop_size;
    let mut block = vec![0.0; hop * 2];
    let mut mono = vec![0.0; hop];

    let mut received: HashMap<String, Vec<f32>> = HashMap::new();
    let mut buf = [0u8; 1536];
    let recv_all = |received: &mut HashMap<String, Vec<f32>>, buf: &mut [u8]| {
        receiver.set_nonblocking(true).unwrap();
        while let Ok(n) = receiver.recv(buf) {
            if let Ok((_, rosc::OscPacket::Message(m))) = rosc::decoder::decode_udp(&buf[..n])
                && let Some(rosc::OscType::Float(v)) = m.args.first()
            {
                received.entry(m.addr).or_default().push(*v);
            }
        }
    };
    while src.read_block(&mut block) {
        for (i, m) in mono.iter_mut().enumerate() {
            *m = 0.5 * (block[2 * i] + block[2 * i + 1]);
        }
        sender.send(g, an.process(&mono));
        sender.flush(false).unwrap();
        recv_all(&mut received, &mut buf);
    }
    std::thread::sleep(Duration::from_millis(100));
    recv_all(&mut received, &mut buf);

    for addr in [
        "/av/mix/level",
        "/av/drums/kick",
        "/av/band/bass",
        "/av/tempo/bpm",
        "/av/lead/note",
    ] {
        assert!(received.contains_key(addr), "{addr}");
    }
    let mut hz = received["/av/lead/pitch_hz"].clone();
    let tail: Vec<f32> = hz.split_off(hz.len() - 20);
    let mut tail = tail;
    tail.sort_by(f32::total_cmp);
    assert!((tail[10] - 220.0).abs() < 10.0, "{}", tail[10]);
    std::fs::remove_dir_all(&dir).ok();
}
