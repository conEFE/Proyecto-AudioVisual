import numpy as np

from visualizer.analyzer import LiveAnalyzer
from visualizer.config import load_config

SR = 44100


def run(signal, cfg=None):
    cfg = cfg or load_config()
    an = LiveAnalyzer(SR, cfg)
    hop = cfg["analysis"]["hop_size"]
    frames = []
    for i in range(0, len(signal) - hop + 1, hop):
        frames.append(an.process(signal[i:i + hop]))
    return an, frames


def tone(freq, seconds, amp=0.3):
    t = np.arange(int(SR * seconds)) / SR
    # tono con armónicos, como un instrumento real
    return sum(amp / h * np.sin(2 * np.pi * freq * h * t) for h in (1, 2, 3, 4)).astype(np.float32)


def kick_track(bpm, seconds):
    out = np.zeros(int(SR * seconds), dtype=np.float32)
    period = int(SR * 60 / bpm)
    n = int(0.15 * SR)
    t = np.arange(n) / SR
    freq = 50 + 100 * np.exp(-t * 30)
    kick = 0.9 * np.sin(2 * np.pi * np.cumsum(freq) / SR) * np.exp(-t * 20)
    for start in range(0, len(out) - n, period):
        out[start:start + n] += kick
    return out


def hihat_track(bpm, seconds, seed=0):
    rng = np.random.default_rng(seed)
    out = np.zeros(int(SR * seconds), dtype=np.float32)
    period = int(SR * 60 / bpm)
    n = int(0.03 * SR)
    noise = rng.standard_normal(n)
    hp = np.diff(noise, prepend=0) * np.exp(-np.arange(n) / SR * 150)
    for start in range(period // 2, len(out) - n, period):
        out[start:start + n] += 0.3 * hp
    return out


def test_pitch_a440():
    _, frames = run(tone(440, 1.5))
    hz = np.median([f["lead/pitch_hz"] for f in frames[-40:]])
    assert abs(hz - 440) < 8
    assert frames[-1]["lead/pitch_class"] == 9  # La
    assert frames[-1]["chroma/a"] > 0.9


def test_bass_vs_lead():
    _, low = run(tone(80, 1.0))
    _, high = run(tone(880, 1.0))
    assert low[-1]["band/bass"] > low[-1]["band/high"]
    assert high[-1]["band/mid"] > 0.5
    assert low[-1]["band/bass"] > 0.5


def test_kick_onsets_and_tempo():
    an, frames = run(kick_track(120, 10))
    fps = SR / an.hop
    hits = np.array([f["drums/kick"] for f in frames]) == 1.0
    n_hits = hits.sum()
    assert 17 <= n_hits <= 21  # 20 bombos en 10 s
    assert abs(frames[-1]["tempo/bpm"] - 120) < 4
    assert frames[-1]["mix/percussive"] > 0
    assert sum(f["drums/snare"] == 1.0 for f in frames) <= 2
    assert sum(f["drums/hihat"] == 1.0 for f in frames) <= 2
    assert fps > 80


def test_hihat_detected_not_kick():
    _, frames = run(hihat_track(120, 4))
    hh = sum(f["drums/hihat"] == 1.0 for f in frames)
    kick = sum(f["drums/kick"] == 1.0 for f in frames)
    assert hh >= 6
    assert kick <= 1


def test_silence_is_stable():
    _, frames = run(np.zeros(SR, dtype=np.float32))
    f = frames[-1]
    for k, v in f.items():
        assert np.isfinite(v), k
    assert f["mix/level"] == 0
    assert f["drums/kick"] == 0


def test_values_normalized():
    sig = kick_track(100, 3) + tone(220, 3) + hihat_track(100, 3)
    _, frames = run(sig)
    unbounded = {"tempo/bpm", "lead/pitch_hz", "lead/note", "lead/pitch_class", "mix/centroid_hz"}
    for f in frames:
        for k, v in f.items():
            if k not in unbounded:
                assert -1e-6 <= v <= 1 + 1e-6, (k, v)


def snare_track(bpm, seconds, seed=1):
    rng = np.random.default_rng(seed)
    out = np.zeros(int(SR * seconds), dtype=np.float32)
    period = int(SR * 60 / bpm)
    n = int(0.12 * SR)
    t = np.arange(n) / SR
    hit = (0.5 * np.sin(2 * np.pi * 200 * t) + 0.4 * rng.standard_normal(n)) * np.exp(-t * 35)
    for start in range(0, len(out) - n, period):
        out[start:start + n] += hit
    return out


def test_snare_detected():
    _, frames = run(snare_track(120, 4))
    assert sum(f["drums/snare"] == 1.0 for f in frames) >= 6


def test_realtime_budget():
    import time
    cfg = load_config()
    an = LiveAnalyzer(SR, cfg)
    sig = kick_track(120, 2) + tone(330, 2)
    hop = cfg["analysis"]["hop_size"]
    t0 = time.perf_counter()
    n = 0
    for i in range(0, len(sig) - hop, hop):
        an.process(sig[i:i + hop])
        n += 1
    per_frame = (time.perf_counter() - t0) / n
    assert per_frame < hop / SR  # más rápido que tiempo real
