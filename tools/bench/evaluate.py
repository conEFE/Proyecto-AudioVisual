#!/usr/bin/env python3
"""Mide qué tan bien detecta el analizador cada instrumento, contra la "respuesta correcta".

  python tools/bench/evaluate.py CARPETA_CANCIONES            # usa el analizador en Rust
  python tools/bench/evaluate.py CARPETA --python             # usa visualizer/analyzer.py

Golpes (bombo, caja, hi-hat): precisión (cuántos detectados eran reales), recall (cuántos reales
se detectaron) y F1. Melodía: % de tiempo con la nota correcta. Bajo/melodía: contraste entre
cuando suenan y cuando no.
"""

import csv
import io
import json
import subprocess
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[2]
HOP, SR = 512, 44100
EARLY, LATE = 0.03, 0.08  # ventana de acierto alrededor del golpe real (s)


def rust_features(wav):
    exe = ROOT / "rust/target/release/examples/dump_features"
    out = subprocess.run([str(exe), str(wav)], capture_output=True, text=True, check=True).stdout
    rows = list(csv.reader(io.StringIO(out)))
    names = rows[0]
    data = np.array(rows[1:], dtype=np.float64)
    return {n: data[:, i] for i, n in enumerate(names)}


def python_features(wav):
    sys.path.insert(0, str(ROOT))
    import soundfile as sf
    from visualizer.analyzer import LiveAnalyzer
    from visualizer.config import load_config
    d, sr = sf.read(wav, dtype="float32", always_2d=True)
    mono = d.mean(axis=1)
    cfg = load_config()
    an = LiveAnalyzer(sr, cfg)
    rows = [an.process(mono[i:i + HOP]) for i in range(0, len(mono) - HOP + 1, HOP)]
    return {k: np.array([r[k] for r in rows]) for k in rows[0]}


def match(det, ref, dontcare=()):
    """Empareja detecciones con golpes reales. Devuelve (aciertos, falsos, perdidos, latencias)."""
    det = sorted(det)
    used = [False] * len(det)
    tp, lat = 0, []
    for r in ref:
        best = None
        for j, d in enumerate(det):
            if not used[j] and r - EARLY <= d <= r + LATE:
                if best is None or abs(d - r) < abs(det[best] - r):
                    best = j
        if best is not None:
            used[best] = True
            tp += 1
            lat.append(det[best] - r)
    fp = 0
    for j, d in enumerate(det):
        if not used[j] and not any(c - EARLY <= d <= c + LATE for c in dontcare):
            fp += 1
    return tp, fp, len(ref) - tp, lat


def active(intervals, times):
    m = np.zeros(len(times), bool)
    for a, b, _ in intervals:
        m |= (times >= a) & (times < b)
    return m


def note_at(intervals, times):
    n = np.full(len(times), -1)
    for a, b, note in intervals:
        n[(times >= a) & (times < b)] = note
    return n


def evaluate(folder, use_python=False):
    songs = sorted(Path(folder).glob("*.json"))
    totals = {k: [0, 0, 0] for k in ("kick", "snare", "hihat")}
    lats = {k: [] for k in totals}
    pitch_ok = pitch_n = 0
    contrast = {"bass": [], "lead": []}
    print(f"{'canción':<22}{'bombo F1':>9}{'caja F1':>9}{'hihat F1':>9}{'nota %':>8}")
    for js in songs:
        lab = json.loads(js.read_text())
        wav = js.with_suffix(".wav")
        f = python_features(wav) if use_python else rust_features(wav)
        n = len(f["mix/level"])
        times = (np.arange(n) + 1) * HOP / SR  # fin de cada bloque = cuando se conoce el valor
        row = [f"{lab['name'][:21]:<22}"]
        for key, feat, dc in (("kick", "drums/kick", []), ("snare", "drums/snare", lab["ghost"]),
                              ("hihat", "drums/hihat", lab["cymbal"])):
            det = times[f[feat] >= 0.999]
            tp, fp, fn, lat = match(det, lab[key], dc)
            for i, v in enumerate((tp, fp, fn)):
                totals[key][i] += v
            lats[key] += lat
            p = tp / max(tp + fp, 1); r = tp / max(tp + fn, 1)
            row.append(f"{2 * p * r / max(p + r, 1e-9):9.2f}" if lab[key] else f"{'-':>9}")
        # melodía: nota (sin octava) mientras suena una sola nota de melodía
        ref = note_at(lab["lead"], times)
        voiced = (ref >= 0) & (f["lead/pitch_hz"] > 0)
        mask = ref >= 0
        ok = (np.round(f["lead/note"]) % 12 == ref % 12) & voiced
        pitch_ok += ok.sum(); pitch_n += mask.sum()
        row.append(f"{100 * ok.sum() / max(mask.sum(), 1):8.0f}")
        for key in ("bass", "lead"):
            act = active(lab[key], times)
            if act.any() and (~act).any():
                contrast[key].append(f[key][act].mean() - f[key][~act].mean())
        print("".join(row))
    print("-" * 57)
    print(f"{'TOTAL':<10}{'precisión':>11}{'recall':>9}{'F1':>7}{'latencia':>11}")
    f1s = {}
    for key, name in (("kick", "bombo"), ("snare", "caja"), ("hihat", "hi-hat")):
        tp, fp, fn = totals[key]
        p = tp / max(tp + fp, 1); r = tp / max(tp + fn, 1); f1 = 2 * p * r / max(p + r, 1e-9)
        f1s[key] = f1
        l = np.median(lats[key]) * 1000 if lats[key] else float("nan")
        print(f"{name:<10}{p:11.2f}{r:9.2f}{f1:7.2f}{l:9.0f} ms")
    print(f"melodía: nota correcta {100 * pitch_ok / max(pitch_n, 1):.0f}% del tiempo")
    for key, name in (("bass", "bajo"), ("lead", "melodía")):
        print(f"contraste {name} (suena - no suena): {np.mean(contrast[key]):+.2f}")
    return f1s, pitch_ok / max(pitch_n, 1)


if __name__ == "__main__":
    evaluate(sys.argv[1], use_python="--python" in sys.argv)
