"""Detector de golpes de batería (bombo, caja, hi-hat) calibrado con datos.

Por cada bloque mide, en 15 zonas de frecuencia, cuánto subió el sonido (flujo) y cuánto se
sostiene, y combina esos números con pesos aprendidos (ver tools/bench). Los pesos están en
`drum_model.json`, que comparten esta versión y la de Rust.

La caja y el bombo esperan ~25 ms antes de decidir: así distinguen "caja + hi-hat" de
"bombo + hi-hat", que en el primer instante se parecen mucho.
"""

import json
from collections import deque
from pathlib import Path

import numpy as np

MODEL_PATH = Path(__file__).resolve().parent.parent / "drum_model.json"


def load_model(path=MODEL_PATH):
    with open(path, encoding="utf-8") as f:
        return json.load(f)


class DrumDetector:
    def __init__(self, model: dict, freqs: np.ndarray, fps: float):
        edges = model["edges"]
        self.masks = [(freqs >= a) & (freqs < b) for a, b in zip(edges[:-1], edges[1:])]
        nb = len(self.masks)
        self.gain = model["gain"]
        self.level_decay = float(np.exp(-1.0 / (model["level_seconds"] * fps)))
        self.peak = 1e-4
        self.rel_a = float(np.exp(-1.0 / (model["rel_seconds"] * fps)))
        self.rel_eps = model["rel_eps"]
        self.rel_mean = np.full(2 * nb, model["rel_init"])
        self.prev = np.zeros(2 * nb)
        self.models = {}
        max_k = 0
        for key, m in model["models"].items():
            k = m["context"]
            max_k = max(max_k, k)
            self.models[key] = {
                "k": k,
                "features": m["features"],
                "w": np.asarray(m["weights"], dtype=np.float64),
                "b": m["bias"],
                "threshold": m["threshold"],
                "offset": 0.0,
                "rows": deque([None] * (k + 1), maxlen=k + 1),
                "prev": 0.0,
                "cool": 0,
                "cooldown": int(m.get("refractory", model["refractory"]) * fps),
            }
        self.levels = deque(maxlen=max_k + 2)
        self.scores = {k: 0.0 for k in self.models}
        self.triggered = {k: False for k in self.models}

    def set_sensitivity(self, key: str, s: float):
        """s en 0..1 (0.5 = calibrado). Más alto = detecta golpes más suaves."""
        self.models[key]["offset"] = (float(s) - 0.5) * 6.0

    def features(self, perc: np.ndarray, mag: np.ndarray):
        """Calcula (flujo, flujo relativo, nivel) de este bloque y actualiza el estado."""
        self.peak = max(float(mag.mean()), self.peak * self.level_decay, 1e-6)
        scale = self.gain / self.peak
        p = np.array([np.log1p(scale * perc[m].mean()) if m.any() else 0.0 for m in self.masks])
        q = np.array([np.log1p(scale * mag[m].mean()) if m.any() else 0.0 for m in self.masks])
        lv = np.concatenate([p, q])
        flux = np.maximum(lv - self.prev, 0.0)
        self.prev = lv
        rel = flux / (self.rel_mean + self.rel_eps)
        self.rel_mean = self.rel_a * self.rel_mean + (1.0 - self.rel_a) * flux
        if not self.levels:
            self.levels.extend([lv] * self.levels.maxlen)
        self.levels.append(lv)
        return flux, rel, lv

    def process(self, perc: np.ndarray, mag: np.ndarray) -> dict:
        flux, rel, lv = self.features(perc, mag)

        for key, m in self.models.items():
            parts = []
            if "flux" in m["features"]:
                parts.append(flux)
            if "rel" in m["features"]:
                parts.append(rel)
            if "lvl" in m["features"]:
                parts.append(lv - self.levels[-(m["k"] + 2)])
            m["rows"].append(np.concatenate(parts))
            x = np.concatenate([r if r is not None else np.zeros_like(m["rows"][-1])
                                for r in reversed(m["rows"])])
            score = float(m["w"] @ x + m["b"])
            if m["cool"] > 0:
                m["cool"] -= 1
            hit = score > m["threshold"] - m["offset"] and m["cool"] == 0 and score >= m["prev"]
            if hit:
                m["cool"] = m["cooldown"]
            m["prev"] = score
            self.scores[key] = score
            self.triggered[key] = hit
        return self.triggered
