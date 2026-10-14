#!/usr/bin/env python3
"""Genera canciones de prueba con instrumentos reales (MIDI + soundfont) y su "respuesta correcta".

Cada canción sale como WAV + JSON con los tiempos exactos de cada golpe de bombo, caja y hi-hat,
y las notas del bajo y de la melodía. Sirve para medir qué tan bien detecta el analizador.

  python tools/bench/make_songs.py salida/ [cantidad] [semilla] [--sintetizada] [--sueltos]
  (requiere fluidsynth y un soundfont GM; --sintetizada usa baterías sintetizadas al azar)
"""

import json
import random
import subprocess
import sys
from pathlib import Path

import mido
import numpy as np
import soundfile as sf

SF2 = "/usr/share/sounds/sf2/FluidR3_GM.sf2"
TPB = 480  # ticks por negra

KICK, SNARE, CLAP, HAT_C, HAT_P, HAT_O, CRASH, RIDE = 36, 38, 39, 42, 44, 46, 49, 51
SCALES = {"menor": [0, 2, 3, 5, 7, 8, 10], "mayor": [0, 2, 4, 5, 7, 9, 11]}

# estilo: (bpm, patrones por paso de 16avo en un compás) — K bombo, S caja, H hi-hat cerrado,
# O hi-hat abierto, g caja fantasma (suave)
STYLES = {
    "rock": dict(bpm=(110, 135), k="x.......x.x.....", s="....x.......x...", h="x.x.x.x.x.x.x.x.", o="", g=""),
    "house": dict(bpm=(120, 128), k="x...x...x...x...", s="....x.......x...", h="", o="..x...x...x...x.", g="", clap=True),
    "hiphop": dict(bpm=(84, 96), k="x.....x...x..x..", s="....x.......x...", h="xxxxxxxxxxxxxxxx", o="", g=""),
    "funk": dict(bpm=(96, 112), k="x..x....x.x...x.", s="....x.......x...", h="xxxxxxxxxxxxxxxx", o="", g=".g.....g..g....g"),
    "balada": dict(bpm=(70, 84), k="x.......x.......", s="....x.......x...", h="x.x.x.x.x.x.x.x.", o="", g="", ride=True),
    "dnb": dict(bpm=(160, 174), k="x.........x.....", s="....x.......x...", h="x.x.x.x.x.x.x.x.", o="", g=""),
    # golpes sueltos al azar, sin otros instrumentos (batería sola, silencios)
    "sueltos": dict(bpm=(70, 140), k="", s="", h="", o="", g="", solo=True),
}
LEADS = {"sinte": 81, "voz": 53, "guitarra": 27, "piano": 0, "saxo": 65, "flauta": 73}
BASSES = [33, 34, 38, 39]
PADS = [48, 89, 0, 19]
DRUM_KITS = [0, 8, 16, 24, 25, 32]  # estándar, room, power, electrónica, TR-808, jazz


def make_song(name, style, lead_name, seed, bars=12, synth_drums=False):
    rnd = random.Random(seed)
    # algunas canciones tienen un "break" de 2 compases con batería sola
    break_bars = {6, 7} if rnd.random() < 0.5 else set()
    st = STYLES[style]
    bpm = rnd.randint(*st["bpm"])
    sec_per_tick = 60.0 / bpm / TPB
    root = rnd.choice([40, 41, 43, 45, 47])  # Mi, Fa, Sol, La, Si graves
    scale = SCALES[rnd.choice(list(SCALES))]
    prog = rnd.choice([[0, 5, 3, 4], [0, 3, 4, 4], [0, 5, 2, 6], [0, 6, 5, 4]])
    events = []  # (tick, canal, nota, vel, dur)
    labels = {"kick": [], "snare": [], "hihat": [], "ghost": [], "cymbal": [], "bass": [], "lead": [], "chords": []}
    step = TPB // 4

    def deg(d, octave=0):
        return root + scale[d % 7] + 12 * (d // 7 + octave)

    lead_deg = 7 + 4
    for bar in range(bars):
        t0 = bar * 16 * step
        chord = prog[bar % 4]
        drums = bar >= 2  # la batería entra en el compás 3
        fill = drums and bar % 4 == 3 and rnd.random() < 0.6
        if st.get("solo"):
            for i in range(16):
                if rnd.random() < 0.55:
                    continue
                t = t0 + i * step
                v = rnd.randint(50, 127)
                for inst, prob in (("kick", 0.4), ("snare", 0.3), ("hihat", 0.45)):
                    if rnd.random() < prob:
                        note = {"kick": KICK, "snare": SNARE, "hihat": rnd.choice([HAT_C, HAT_C, HAT_O])}[inst]
                        events.append((t, 9, note, v, step)); labels[inst].append(t)
            continue
        for i in range(16):
            t = t0 + i * step
            vel = lambda base: max(30, min(127, base + rnd.randint(-12, 8)))
            if drums:
                if st["k"][i] == "x" or (fill and i >= 12 and rnd.random() < 0.3):
                    events.append((t, 9, KICK, vel(110), step)); labels["kick"].append(t)
                if st["s"][i] == "x" or (fill and i >= 12 and i % 2 == 0):
                    n = CLAP if st.get("clap") else SNARE
                    events.append((t, 9, n, vel(105), step)); labels["snare"].append(t)
                if st["g"] and st["g"][i] == "g":
                    events.append((t, 9, SNARE, 38, step)); labels["ghost"].append(t)
                if st["h"] and st["h"][i] == "x":
                    v = vel(90 if i % 4 == 0 else 65)
                    n = RIDE if st.get("ride") else HAT_C
                    events.append((t, 9, n, v, step)); labels["hihat"].append(t)
                if st["o"] and st["o"][i] == "x":
                    events.append((t, 9, HAT_O, vel(85), step * 2)); labels["hihat"].append(t)
                if i == 0 and bar % 4 == 0 and bar > 2:
                    events.append((t, 9, CRASH, 100, step * 8)); labels["cymbal"].append(t)
            if bar in break_bars:
                continue
            # bajo: sigue al bombo + raíz en el 1
            if bar >= 1 and (i == 0 or (st["k"][i] == "x" and rnd.random() < 0.7) or (i == 10 and rnd.random() < 0.4)):
                n = deg(chord) if rnd.random() < 0.8 else deg(chord + 4)
                dur = step * rnd.choice([2, 3, 4])
                events.append((t, 0, n, 100, dur)); labels["bass"].append([t, t + dur, n])
            # melodía: frases de 2 compases con silencios
            if bar >= 1 and i % 2 == 0 and (bar % 2 == 0 or i < 10) and rnd.random() < 0.6:
                lead_deg = max(7, min(18, lead_deg + rnd.choice([-2, -1, -1, 0, 1, 1, 2])))
                n = deg(lead_deg, 1)
                dur = step * rnd.choice([2, 2, 3, 4])
                events.append((t, 1, n, 95, dur)); labels["lead"].append([t, t + dur, n])
        # acordes: uno por compás, suave
        for d in ((0, 2, 4) if bar not in break_bars else ()):
            n = deg(chord + d, 1)
            events.append((t0, 2, n, 55, 16 * step)); labels["chords"].append([t0, t0 + 16 * step, n])

    mid = mido.MidiFile(ticks_per_beat=TPB)
    tr = mido.MidiTrack(); mid.tracks.append(tr)
    tr.append(mido.MetaMessage("set_tempo", tempo=mido.bpm2tempo(bpm), time=0))
    kit = rnd.choice(DRUM_KITS)
    for ch, prog_n in ((0, rnd.choice(BASSES)), (1, LEADS[lead_name]), (2, rnd.choice(PADS)), (9, kit)):
        tr.append(mido.Message("program_change", channel=ch, program=prog_n, time=0))
    # mezcla al azar, como en canciones reales
    for ch, lo, hi in ((9, 80, 127), (0, 80, 120), (1, 75, 120), (2, 50, 90)):
        tr.append(mido.Message("control_change", channel=ch, control=7, value=rnd.randint(lo, hi), time=0))
    msgs = []
    drum_hits = [(t, n, v) for t, ch, n, v, dur in events if ch == 9]
    if synth_drums:
        events = [e for e in events if e[1] != 9]
    for t, ch, n, v, dur in events:
        msgs.append((t, 1, mido.Message("note_on", channel=ch, note=n, velocity=v)))
        msgs.append((t + dur, 0, mido.Message("note_off", channel=ch, note=n, velocity=0)))
    msgs.sort(key=lambda m: (m[0], m[1]))
    last = 0
    for t, _, m in msgs:
        tr.append(m.copy(time=t - last)); last = t

    sec = lambda tick: round(tick * sec_per_tick, 4)
    out = {
        "name": name, "style": style, "lead": lead_name, "bpm": bpm,
        "kit": "sintetizada" if synth_drums else kit,
        "kick": [sec(t) for t in labels["kick"]],
        "snare": [sec(t) for t in labels["snare"]],
        "hihat": [sec(t) for t in labels["hihat"]],
        "ghost": [sec(t) for t in labels["ghost"]],
        "cymbal": [sec(t) for t in labels["cymbal"]],
        "bass": [[sec(a), sec(b), n] for a, b, n in labels["bass"]],
        "lead": [[sec(a), sec(b), n] for a, b, n in labels["lead"]],
        "chords": [[sec(a), sec(b), n] for a, b, n in labels["chords"]],
    }
    out["_drum_hits"] = [[sec(t), n, v] for t, n, v in drum_hits]
    return mid, out


def synth_kit(rnd):
    """Parámetros al azar de una batería sintetizada (808, acústica, electrónica...)."""
    return dict(
        k_f0=rnd.uniform(70, 220), k_f1=rnd.uniform(38, 65), k_sweep=rnd.uniform(15, 60),
        k_decay=rnd.uniform(5, 18), k_click=rnd.uniform(0, 0.5), k_dist=rnd.uniform(1, 3),
        s_tone=rnd.uniform(140, 260), s_tone_amt=rnd.uniform(0.2, 0.8), s_noise_lo=rnd.uniform(800, 2500),
        s_noise_hi=rnd.uniform(5000, 11000), s_decay=rnd.uniform(12, 35), s_clap=rnd.random() < 0.3,
        h_cut=rnd.uniform(4000, 9000), h_decay=rnd.uniform(40, 150), h_open_decay=rnd.uniform(6, 15),
        gain=rnd.uniform(0.4, 1.0),
    )


def _bandnoise(n, lo, hi, sr, rng):
    x = rng.standard_normal(n)
    X = np.fft.rfft(x)
    f = np.fft.rfftfreq(n, 1 / sr)
    X[(f < lo) | (f > hi)] = 0
    return np.fft.irfft(X, n)


def render_synth_drums(hits, total, sr, kit, seed):
    rng = np.random.default_rng(seed)
    out = np.zeros(total)
    for t, note, vel in hits:
        i0 = int(t * sr)
        amp = (vel / 127) ** 1.5 * kit["gain"]
        if note == KICK:
            n = int(0.5 * sr); tt = np.arange(n) / sr
            f = kit["k_f1"] + (kit["k_f0"] - kit["k_f1"]) * np.exp(-tt * kit["k_sweep"])
            x = np.sin(2 * np.pi * np.cumsum(f) / sr) * np.exp(-tt * kit["k_decay"])
            x = np.tanh(kit["k_dist"] * x) / np.tanh(kit["k_dist"])
            x[: int(0.004 * sr)] += kit["k_click"] * rng.standard_normal(int(0.004 * sr))
        elif note in (SNARE, CLAP):
            n = int(0.4 * sr); tt = np.arange(n) / sr
            noise = _bandnoise(n, kit["s_noise_lo"], kit["s_noise_hi"], sr, rng)
            noise /= np.abs(noise).max() + 1e-9
            env = np.exp(-tt * kit["s_decay"])
            if kit["s_clap"] or note == CLAP:
                for d in (0.0, 0.011, 0.022):
                    env += 0.8 * np.exp(-np.maximum(tt - d, 0) * 120) * (tt >= d)
            x = noise * env + kit["s_tone_amt"] * np.sin(2 * np.pi * kit["s_tone"] * tt) * np.exp(-tt * 30)
            if note == SNARE and vel < 50:
                x *= 0.4
        elif note in (HAT_C, HAT_P, HAT_O, RIDE):
            dec = kit["h_open_decay"] if note == HAT_O else kit["h_decay"]
            n = int(0.6 * sr); tt = np.arange(n) / sr
            x = _bandnoise(n, kit["h_cut"], 18000, sr, rng)
            x = x / (np.abs(x).max() + 1e-9) * np.exp(-tt * dec) * 0.5
        elif note == CRASH:
            n = int(1.5 * sr); tt = np.arange(n) / sr
            x = _bandnoise(n, 3000, 18000, sr, rng)
            x = x / (np.abs(x).max() + 1e-9) * np.exp(-tt * 2.5) * 0.4
        else:
            continue
        seg = out[i0:i0 + len(x)]
        seg += amp * x[: len(seg)]
    return out


def main():
    outdir = Path(sys.argv[1] if len(sys.argv) > 1 else "bench_songs")
    count = int(sys.argv[2]) if len(sys.argv) > 2 else 10
    seed0 = int(sys.argv[3]) if len(sys.argv) > 3 else 1000
    synth = "--sintetizada" in sys.argv
    outdir.mkdir(parents=True, exist_ok=True)
    solo = "--sueltos" in sys.argv
    styles = ["sueltos"] if solo else [k for k in STYLES if k != "sueltos"]
    leads = list(LEADS)
    for i in range(count):
        style = styles[i % len(styles)]
        lead = leads[(i * 5 + seed0) % len(leads)]
        name = f"{'s' if synth else ''}{i:02d}_{style}" + ("" if solo else f"_{lead}")
        mid, labels = make_song(name, style, lead, seed=seed0 + i, synth_drums=synth)
        mid.save(outdir / f"{name}.mid")
        wav = outdir / f"{name}.wav"
        subprocess.run(["fluidsynth", "-ni", "-q", "-F", str(wav), "-r", "44100", "-g", "0.7",
                        SF2, str(outdir / f"{name}.mid")], check=True, stdout=subprocess.DEVNULL)
        hits = labels.pop("_drum_hits")
        if synth:
            audio, sr = sf.read(wav, always_2d=True)
            # sin otros instrumentos el render puede quedar corto: alargar hasta el último golpe
            total = max(len(audio), int((max((h[0] for h in hits), default=0) + 1.5) * sr))
            audio = np.vstack([audio, np.zeros((total - len(audio), audio.shape[1]))])
            kit = synth_kit(random.Random(seed0 * 7 + i))
            drums = render_synth_drums(hits, len(audio), sr, kit, seed0 + i)
            audio = audio + drums[:, None] * 0.6
            audio /= max(1.0, np.abs(audio).max() / 0.95)
            sf.write(wav, audio, sr)
        (outdir / f"{name}.json").write_text(json.dumps(labels))
        print(name, labels["bpm"], "BPM, batería", labels["kit"])


if __name__ == "__main__":
    main()
