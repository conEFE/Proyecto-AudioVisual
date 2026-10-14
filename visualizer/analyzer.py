"""Análisis de audio en tiempo real.

Por cada bloque de audio (hop) calcula:
  * nivel general, brillo y "ruidosidad" del sonido
  * energía por bandas de frecuencia (configurables)
  * separación armónico/percusiva (HPSS) en tiempo real
  * detectores de instrumentos aproximados: bombo, caja, hi-hat, bajo, melodía/voz
  * altura (nota) de la melodía y cromagrama (12 notas)
  * tempo (BPM), fase del pulso y golpe de pulso

Todo lo que sale está normalizado a 0..1 salvo BPM, Hz y nota MIDI.
"""

from collections import deque

import numpy as np
from numpy.lib.stride_tricks import sliding_window_view

from .drums import DrumDetector, load_model
from .dsp import AutoGain, Envelope, OnsetDetector

NOTE_NAMES = ["c", "cs", "d", "ds", "e", "f", "fs", "g", "gs", "a", "as", "b"]


class LiveAnalyzer:
    def __init__(self, sample_rate: int, config: dict):
        a = config["analysis"]
        self.sr = sample_rate
        # hop y ventana están definidos a 44.1 kHz: se escalan para que cada bloque dure lo mismo
        # a cualquier frecuencia de muestreo (los detectores calibrados dependen de eso)
        scale = sample_rate / 44100.0
        self.hop = max(1, int(round(a["hop_size"] * scale)))
        self.win = 2 * int(round(a["window_size"] * scale / 2))
        self.nfft = self.win * 2  # zero-padding: mejor resolución para la nota
        self.fps = self.sr / self.hop
        self.window = np.hanning(self.win).astype(np.float32)
        self.buffer = np.zeros(self.win, dtype=np.float32)
        self.freqs = np.fft.rfftfreq(self.nfft, 1.0 / self.sr)
        nbins = len(self.freqs)

        # HPSS en tiempo real: mediana en el tiempo (armónico) y en frecuencia (percusivo)
        self.hpss_kernel = a["hpss_kernel"]
        self.mag_history = deque(maxlen=self.hpss_kernel)
        self.prev_perc = np.zeros(nbins, dtype=np.float32)
        self.prev_mag = np.zeros(nbins, dtype=np.float32)

        gain_decay = a["autogain_seconds"]
        att, rel = a["attack"], a["release"]
        env = lambda: Envelope(att, rel, self.fps)
        gain = lambda: AutoGain(gain_decay, self.fps)

        self.bands = [(b["name"], b["low"], b["high"]) for b in config["bands"]]
        self.band_masks = {n: self._mask(lo, hi) for n, lo, hi in self.bands}
        self.band_env = {n: env() for n, _, _ in self.bands}
        self.band_gain = {n: gain() for n, _, _ in self.bands}

        self.inst_cfg = config["instruments"]
        self.inst_masks = {
            name: [self._mask(lo, hi) for lo, hi in spec["ranges"]]
            for name, spec in self.inst_cfg.items()
        }
        self.onsets = {
            name: OnsetDetector(self.fps, sensitivity=spec.get("sensitivity", 1.5),
                                refractory_seconds=spec.get("refractory", 0.08),
                                pulse_release=spec.get("pulse_release", 0.15),
                                min_flux=spec.get("min_flux", 0.05))
            for name, spec in self.inst_cfg.items() if spec["type"] == "percussive"
        }
        # detector calibrado para bombo, caja y hi-hat (si existe drum_model.json)
        self.drums = None
        self.drum_keys = {}
        try:
            model = load_model()
            self.drums = DrumDetector(model, self.freqs, self.fps)
            self.drum_keys = {f"drums/{k}": k for k in model["models"] if f"drums/{k}" in self.onsets}
        except FileNotFoundError:
            pass
        self.drum_pulse = {name: 0.0 for name in self.drum_keys}
        self.drum_decay = {name: Envelope(0.0, self.inst_cfg[name].get("pulse_release", 0.15), self.fps).r
                           for name in self.drum_keys}
        self.inst_env = {name: env() for name in self.inst_cfg}
        # claves que son pulsos (golpes): el emisor OSC no debe perderlas
        self.pulse_keys = set(self.onsets) | {"mix/onset", "tempo/beat"}
        self.inst_gain = {name: gain() for name in self.inst_cfg}

        self.rms_env, self.rms_gain = env(), gain()
        self.centroid_env = env()
        self.flatness_env = env()
        self.perc_env, self.perc_gain = env(), gain()
        self.harm_env, self.harm_gain = env(), gain()
        self.onset_all = OnsetDetector(self.fps, sensitivity=a["onset_sensitivity"])

        # Melodía / nota
        p = config["pitch"]
        self.pitch_mask = self._mask(p["min_hz"], p["max_hz"])
        self.pitch_conf_min = p["min_confidence"]
        self.pitch_harmonics = p.get("harmonics", 8)
        # restar el "fondo" (lo que viene sonando hace ~1 s, como los acordes) resalta la melodía
        self.pitch_bg = p.get("background_subtract", 0.9)
        self.pitch_bg_a = float(np.exp(-1.0 / (p.get("background_seconds", 1.0) * self.fps)))
        self.slow_mag = None
        self.pitch_cand = np.where(self.pitch_mask)[0]
        last = len(self.freqs) - 2
        self.pitch_idx = [np.minimum(self.pitch_cand * h, last) for h in range(1, self.pitch_harmonics + 1)]
        self.pitch_hz = 0.0
        self.chroma_env = [env() for _ in range(12)]
        self.chroma_mask = self._mask(p["min_hz"], 5000)
        with np.errstate(divide="ignore", invalid="ignore"):
            midi = 69 + 12 * np.log2(self.freqs / 440.0)
            self.bin_pitch_class = np.where(self.chroma_mask, np.round(midi) % 12, -1).astype(int)

        # Tempo
        t = config["tempo"]
        self.tempo_min, self.tempo_max = t["min_bpm"], t["max_bpm"]
        self.onset_env_hist = deque(maxlen=int(t["history_seconds"] * self.fps))
        self.tempo_every = max(1, int(t["update_seconds"] * self.fps))
        self.bpm = float(t["initial_bpm"])
        self.phase = 0.0
        self.beat_pulse = 0.0
        self.beat_decay = Envelope(0.0, 0.1, self.fps).r
        self.frame = 0

    def _mask(self, lo: float, hi: float) -> np.ndarray:
        return (self.freqs >= lo) & (self.freqs < hi)

    def process(self, block: np.ndarray) -> dict:
        """Analiza un bloque mono de `hop_size` muestras y devuelve {direccion: valor}."""
        block = np.asarray(block, dtype=np.float32)
        n = len(block)
        self.buffer = np.roll(self.buffer, -n)
        self.buffer[-n:] = block
        self.frame += 1

        mag = np.abs(np.fft.rfft(self.buffer * self.window, n=self.nfft)).astype(np.float32)
        mag /= self.win
        out = {}

        # --- Nivel global y timbre ---
        rms = float(np.sqrt(np.mean(block ** 2)))
        out["mix/level"] = self.rms_env(self.rms_gain(rms))
        power = mag ** 2
        total = power.sum() + 1e-12
        centroid = float((self.freqs * power).sum() / total)
        out["mix/brightness"] = self.centroid_env(min(centroid / 8000.0, 1.0))
        flat = np.exp(np.mean(np.log(mag + 1e-10))) / (np.mean(mag) + 1e-10)
        out["mix/noisiness"] = self.flatness_env(float(flat))
        out["mix/centroid_hz"] = centroid

        # --- Bandas de frecuencia ---
        for name, _, _ in self.bands:
            e = float(np.sqrt(power[self.band_masks[name]].mean() + 1e-20))
            out[f"band/{name}"] = self.band_env[name](self.band_gain[name](e))

        # --- HPSS ---
        self.mag_history.append(mag)
        harm_est = np.median(np.stack(self.mag_history), axis=0)
        k = self.hpss_kernel
        padded = np.pad(mag, (k // 2, k // 2), mode="edge")
        perc_est = np.median(sliding_window_view(padded, k), axis=1)
        h2, p2 = harm_est ** 2, perc_est ** 2
        denom = h2 + p2 + 1e-20
        harm = mag * (h2 / denom)
        perc = mag * (p2 / denom)

        out["mix/harmonic"] = self.harm_env(self.harm_gain(float(np.sqrt((harm ** 2).mean()))))
        out["mix/percussive"] = self.perc_env(self.perc_gain(float(np.sqrt((perc ** 2).mean()))))

        if self.drums is not None:
            self.drums.process(perc, mag)
        flux_all = np.maximum(mag - self.prev_mag, 0.0)
        flux_perc = np.maximum(perc - self.prev_perc, 0.0)
        self.prev_mag, self.prev_perc = mag, perc
        out["mix/onset"] = self.onset_all(float(np.log1p(1000 * flux_all.mean())))
        any_hit = self.onset_all.triggered

        # --- Instrumentos aproximados ---
        drum_flux = 0.0
        for name, spec in self.inst_cfg.items():
            masks = self.inst_masks[name]
            if spec["type"] == "percussive":
                fluxes = [float(np.log1p(1000 * flux_perc[m].mean())) for m in masks]
                # "all": todas las zonas deben golpear a la vez (ej. caja = cuerpo + chasquido)
                flux = min(fluxes) if spec.get("combine") == "all" else sum(fluxes) / len(fluxes)
                drum_flux += flux
                out[f"{name}"] = self.onsets[name](flux)
                if name in self.drum_keys:
                    hit = self.drums.triggered[self.drum_keys[name]]
                    self.onsets[name].triggered = hit
                    self.drum_pulse[name] = 1.0 if hit else self.drum_pulse[name] * self.drum_decay[name]
                    out[f"{name}"] = self.drum_pulse[name]
                energy = sum(float(np.sqrt((perc[m] ** 2).mean())) for m in masks)
                out[f"{name}_level"] = self.inst_env[name](self.inst_gain[name](energy))
            else:
                energy = sum(float(np.sqrt((harm[m] ** 2).mean())) for m in masks)
                out[f"{name}"] = self.inst_env[name](self.inst_gain[name](energy))

        # --- Nota de la melodía (espectro armónico, suma de armónicos) ---
        out.update(self._pitch(harm, mag))

        # --- Tempo y pulso ---
        self.onset_env_hist.append(drum_flux)
        if self.frame % self.tempo_every == 0:
            self._update_tempo()
        kick_hit = "drums/kick" in self.onsets and self.onsets["drums/kick"].triggered
        out.update(self._beat(kick_hit or any_hit))
        return out

    def set_sensitivity(self, name: str, s: float):
        """Sensibilidad 0..1 de un golpe (`drums/kick`, `drums/snare`, `drums/hihat`). 0.5 = calibrado."""
        if name in self.drum_keys:
            self.drums.set_sensitivity(self.drum_keys[name], s)

    def _pitch(self, harm: np.ndarray, mag: np.ndarray) -> dict:
        bg = self.slow_mag
        self.slow_mag = mag.copy() if bg is None else self.pitch_bg_a * bg + (1.0 - self.pitch_bg_a) * mag
        if bg is not None and self.pitch_bg > 0:
            mag = np.maximum(mag - self.pitch_bg * bg, 0.0)
        # prominencia por suma de armónicos (8, con peso decreciente) sobre el espectro completo:
        # la nota que más "explica" lo que suena entre 150 y 1500 Hz
        comp = np.sqrt(mag)
        best = np.maximum(comp, np.maximum(np.roll(comp, 1), np.roll(comp, -1)))
        sal = np.zeros(len(self.pitch_cand))
        for h, idx in enumerate(self.pitch_idx, start=1):
            sal += best[idx] * h ** -0.8
        out = {}
        conf = 0.0
        j = int(np.argmax(sal)) if len(sal) else 0
        peak = sal[j] if len(sal) else 0.0
        if peak > 0:
            conf = float(1.0 - np.mean(sal) / peak)
        if conf >= self.pitch_conf_min and len(sal):
            self.pitch_hz = self._refine_f0(int(self.pitch_cand[j]), mag, comp)
        out["lead/pitch_hz"] = self.pitch_hz
        midi = 69 + 12 * np.log2(self.pitch_hz / 440.0) if self.pitch_hz > 0 else 0.0
        out["lead/note"] = float(midi)
        out["lead/pitch_class"] = float(round(midi) % 12) if self.pitch_hz > 0 else 0.0
        out["lead/confidence"] = conf

        # cromagrama (qué notas suenan)
        p2 = harm ** 2
        chroma = np.bincount(self.bin_pitch_class[self.chroma_mask], weights=p2[self.chroma_mask],
                             minlength=12)
        chroma = chroma / (chroma.max() + 1e-20)
        for i, name in enumerate(NOTE_NAMES):
            out[f"chroma/{name}"] = self.chroma_env[i](float(chroma[i]))
        return out

    def _refine_f0(self, cand: int, mag: np.ndarray, comp: np.ndarray) -> float:
        """Afina la nota con la posición exacta de cada armónico (mucho más preciso que un solo bin)."""
        num = den = 0.0
        last = len(mag) - 2
        for h in range(1, self.pitch_harmonics + 1):
            b = cand * h
            if b >= last:
                break
            k = b - 1 + int(np.argmax(mag[b - 1:b + 2]))
            if not 0 < k < last + 1:
                continue
            a, c0, c = np.log(mag[k - 1:k + 2] + 1e-12)
            d = a - 2 * c0 + c
            shift = float(np.clip(0.5 * (a - c) / d, -0.5, 0.5)) if d != 0 else 0.0
            w = comp[k] * h ** -0.8
            num += w * (k + shift) / h
            den += w
        bin0 = num / den if den > 0 else float(cand)
        return float(bin0 * self.sr / self.nfft)

    def _update_tempo(self):
        x = np.asarray(self.onset_env_hist)
        if len(x) < self.fps * 2 or x.std() < 1e-6:
            return
        x = x - x.mean()
        ac = np.correlate(x, x, mode="full")[len(x) - 1:]
        lags = np.arange(len(ac))
        with np.errstate(divide="ignore"):
            bpm_of_lag = 60.0 * self.fps / lags
        valid = (bpm_of_lag >= self.tempo_min) & (bpm_of_lag <= self.tempo_max)
        if not valid.any():
            return
        # preferencia suave por tempos cercanos a 120 BPM
        weight = np.exp(-0.5 * (np.log2(np.where(valid, bpm_of_lag, 120) / 120.0) / 1.0) ** 2)
        score = np.where(valid, ac * weight, -np.inf)
        lag = int(np.argmax(score))
        if lag <= 0:
            return
        # refinamiento parabólico del lag
        if 1 <= lag < len(ac) - 1:
            a, b, c = ac[lag - 1:lag + 2]
            d = a - 2 * b + c
            lag_f = lag + (0.5 * (a - c) / d if d != 0 else 0.0)
        else:
            lag_f = float(lag)
        new_bpm = 60.0 * self.fps / lag_f
        self.bpm = 0.7 * self.bpm + 0.3 * new_bpm if abs(new_bpm - self.bpm) < 8 else new_bpm

    def _beat(self, hit: bool) -> dict:
        """Oscilador de fase que se re-sincroniza suavemente con los golpes."""
        self.phase += self.bpm / 60.0 / self.fps
        beat = False
        if self.phase >= 1.0:
            self.phase -= 1.0
            beat = True
        if hit:
            err = self.phase if self.phase < 0.5 else self.phase - 1.0
            self.phase -= 0.25 * err
            self.phase %= 1.0
        self.beat_pulse = 1.0 if beat else self.beat_pulse * self.beat_decay
        return {"tempo/bpm": self.bpm, "tempo/phase": self.phase, "tempo/beat": self.beat_pulse}
