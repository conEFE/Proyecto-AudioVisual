#!/usr/bin/env python3
"""Calibra los detectores de bombo, caja y hi-hat y escribe drum_model.json.

  python tools/bench/train_drums.py --entrenar DIR1 DIR2 ... --probar DIR3 DIR4 ...

Las carpetas salen de make_songs.py. Con --probar mide el resultado en canciones que el ajuste
no vio. Al final recalibra con todas las canciones y guarda el modelo.
"""

import argparse
import json
import sys
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

import numpy as np
from sklearn.linear_model import LogisticRegression

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(Path(__file__).resolve().parent))
from evaluate import match  # noqa: E402

EDGES = [30, 60, 100, 150, 200, 300, 450, 700, 1000, 1500, 2200, 3300, 5000, 7500, 11000, 16000]
BASE = {"edges": EDGES, "gain": 1000.0, "level_seconds": 10.0, "rel_seconds": 1.0, "rel_eps": 0.05,
        "rel_init": 0.05, "refractory": 0.06}
# instrumento: (cuadros de contexto, características, ventana de decisión tras el golpe (s), ignorar,
# regularización). La caja espera más (~38 ms): así separa "caja + hi-hat" de "bombo + hi-hat".
CONFIG = {
    "kick": (3, ["flux", "rel", "lvl"], (0.023, 0.058), [], 0.1),
    "snare": (4, ["flux", "rel", "lvl"], (0.035, 0.07), ["ghost"], 0.5),
    "hihat": (2, ["flux", "rel"], (0.0, 0.035), ["cymbal"], 0.1),
}
# tiempo mínimo entre dos golpes del mismo instrumento (s)
REFRACTORY = {"kick": 0.09, "snare": 0.08, "hihat": 0.05}


def extract(wav):
    """Características por bloque, calculadas con el mismo código que usa la app."""
    import soundfile as sf
    from visualizer.analyzer import LiveAnalyzer
    from visualizer.config import load_config
    from visualizer.drums import DrumDetector
    d, sr = sf.read(wav, dtype="float32", always_2d=True)
    x = d.mean(axis=1)
    an = LiveAnalyzer(sr, load_config())
    feat = DrumDetector({**BASE, "models": {}}, an.freqs, an.fps)
    rows = []
    for i in range(0, len(x) - an.hop + 1, an.hop):
        an.process(x[i:i + an.hop])
        flux, rel, lv = feat.features(an.prev_perc, an.prev_mag)
        rows.append(np.concatenate([flux, rel, lv]))
    return np.array(rows), sr, an.hop


def load(folders):
    jsons = [j for f in folders for j in sorted(Path(f).glob("*.json"))]
    with ProcessPoolExecutor() as ex:
        feats = list(ex.map(extract, [j.with_suffix(".wav") for j in jsons]))
    return [(json.loads(j.read_text()), F, sr, hop) for j, (F, sr, hop) in zip(jsons, feats)]


def design(F, k, use):
    n = F.shape[1] // 3
    flux, rel, lv = F[:, :n], F[:, n:2 * n], F[:, 2 * n:]
    parts = []
    if "flux" in use:
        parts.append(flux)
    if "rel" in use:
        parts.append(rel)
    if "lvl" in use:
        prev = np.vstack([np.repeat(lv[:1], k + 1, 0), lv[:-(k + 1)]])
        parts.append(lv - prev)
    X = np.hstack(parts)
    ctx = [X] + [np.vstack([np.zeros((j, X.shape[1])), X[:-j]]) for j in range(1, k + 1)]
    return np.hstack(ctx)


def targets(lab, key, n, sr, hop, window, ignore):
    t = (np.arange(n) + 1) * hop / sr
    d0, d1 = window
    y = np.zeros(n, bool)
    skip = np.zeros(n, bool)
    for o in lab[key]:
        y |= (t >= o + d0) & (t < o + d1)
        skip |= ((t >= o - 0.02) & (t < o + d0)) | ((t >= o + d1) & (t < o + d1 + 0.05))
    for name in ignore:
        for o in lab[name]:
            skip |= (t >= o - 0.02) & (t < o + 0.09)
    return y, skip


def detect(score, sr, hop, thr, refractory):
    out, cool, prev = [], 0, 0.0
    cool_frames = int(refractory * sr / hop)
    for i, v in enumerate(score):
        if cool > 0:
            cool -= 1
        if v > thr and cool == 0 and v >= prev:
            out.append((i + 1) * hop / sr)
            cool = cool_frames
        prev = v
    return out


def fit(data, key):
    k, use, window, ignore, c = CONFIG[key]
    X, Y = [], []
    for lab, F, sr, hop in data:
        y, skip = targets(lab, key, len(F), sr, hop, window, ignore)
        X.append(design(F, k, use)[~skip])
        Y.append(y[~skip])
    model = LogisticRegression(C=c, class_weight="balanced", max_iter=5000).fit(np.vstack(X), np.concatenate(Y))
    best = max((score(model, data, key, t)[0], t) for t in np.linspace(-2, 12, 57))
    return model, best[1]


def score(model, data, key, thr):
    k, use, _, ignore, _ = CONFIG[key]
    tp = fp = fn = 0
    for lab, F, sr, hop in data:
        s = model.decision_function(design(F, k, use))
        a, b, c, _ = match(detect(s, sr, hop, thr, REFRACTORY[key]), lab[key], sum((lab[x] for x in ignore), []))
        tp += a; fp += b; fn += c
    p = tp / max(tp + fp, 1); r = tp / max(tp + fn, 1)
    return 2 * p * r / max(p + r, 1e-9), p, r


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--entrenar", nargs="+", required=True)
    ap.add_argument("--probar", nargs="*", default=[])
    ap.add_argument("--salida", default=str(ROOT / "drum_model.json"))
    args = ap.parse_args()
    train = load(args.entrenar)
    tests = {f: load([f]) for f in args.probar}
    print(f"{len(train)} canciones para calibrar, {sum(map(len, tests.values()))} para probar", flush=True)
    for key in CONFIG:
        model, thr = fit(train, key)
        line = f"{key:<6}"
        for name, data in tests.items():
            f1, p, r = score(model, data, key, thr)
            line += f"  {Path(name).name}: F1 {f1:.2f} (P {p:.2f} R {r:.2f})"
        print(line, flush=True)
    everything = train + [d for v in tests.values() for d in v]
    out = {"_info": "Detectores de golpes calibrados con tools/bench/train_drums.py. No editar a mano.", **BASE,
           "models": {}}
    for key, (k, use, _, _, _) in CONFIG.items():
        model, thr = fit(everything, key)
        out["models"][key] = {"context": k, "features": use, "threshold": float(thr), "refractory": REFRACTORY[key],
                              "bias": round(float(model.intercept_[0]), 6),
                              "weights": [round(float(w), 6) for w in model.coef_[0]]}
    Path(args.salida).write_text(json.dumps(out, indent=1))
    print("guardado", args.salida)


if __name__ == "__main__":
    main()
