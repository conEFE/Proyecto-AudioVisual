#!/usr/bin/env python3
"""Visualizador de audio en vivo → OSC → TouchDesigner.

Ejemplos:
  python live.py                         # micrófono / entrada por defecto
  python live.py --list-devices          # ver dispositivos de audio
  python live.py --device 3 --meter      # usar el dispositivo 3 y ver medidores
  python live.py --file cancion.wav      # probar con un archivo como si fuera en vivo
"""

import argparse
import queue
import sys
import time

import numpy as np

from visualizer.analyzer import LiveAnalyzer
from visualizer.config import load_config
from visualizer.osc_out import OscSender

METER_KEYS = ["mix/level", "band/sub", "band/bass", "band/mid", "band/high",
              "drums/kick", "drums/snare", "drums/hihat", "bass", "lead", "tempo/beat"]


def parse_args():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--config", help="archivo YAML que se aplica encima de config.yaml")
    p.add_argument("--list-devices", action="store_true", help="lista dispositivos de audio y sale")
    p.add_argument("--device", help="índice o nombre del dispositivo de entrada")
    p.add_argument("--channels", type=int, help="cantidad de canales a capturar")
    p.add_argument("--file", help="analiza un archivo de audio simulando entrada en vivo")
    p.add_argument("--no-playback", action="store_true", help="con --file: no reproducir por parlantes")
    p.add_argument("--loop", action="store_true", help="con --file: repetir en bucle")
    p.add_argument("--host", help="IP destino OSC (por defecto 127.0.0.1)")
    p.add_argument("--port", type=int, help="puerto OSC (por defecto 7000)")
    p.add_argument("--meter", action="store_true", help="muestra medidores en la terminal")
    return p.parse_args()


def meter_line(values: dict) -> str:
    parts = []
    for key in METER_KEYS:
        v = values.get(key, 0.0)
        bar = "█" * int(v * 8)
        parts.append(f"{key.split('/')[-1][:5]:>5} {bar:<8}")
    parts.append(f"{values.get('tempo/bpm', 0):5.1f}bpm")
    return " ".join(parts)


def main():
    args = parse_args()
    cfg = load_config(args.config)
    audio, osc = cfg["audio"], cfg["osc"]

    from visualizer import audio_io

    if args.list_devices:
        print(audio_io.list_devices())
        return

    sr = audio["sample_rate"]
    hop = cfg["analysis"]["hop_size"]
    device = args.device if args.device is not None else audio.get("device")
    if isinstance(device, str) and device.isdigit():
        device = int(device)

    inputs = audio.get("inputs") or []
    channels = args.channels or audio.get("channels", 1)
    if inputs:
        channels = max(channels, max(i["channel"] for i in inputs))

    if args.file:
        source = audio_io.FileInput(args.file, sr, hop, playback=not args.no_playback, loop=args.loop)
    else:
        source = audio_io.LiveInput(sr, hop, channels=channels, device=device)

    mix = LiveAnalyzer(sr, cfg)
    per_channel = [(i["name"], i["channel"] - 1, LiveAnalyzer(sr, cfg)) for i in inputs]
    sender = OscSender(args.host or osc["host"], args.port or osc["port"], osc.get("prefix", ""),
                       osc.get("send_fps", 60))

    print(f"Enviando OSC a {args.host or osc['host']}:{args.port or osc['port']} "
          f"({sr} Hz, bloque {hop} = {1000 * hop / sr:.1f} ms). Ctrl+C para salir.", file=sys.stderr)
    if per_channel:
        print("Canales: " + ", ".join(f"{c + 1}→/ch/{n}" for n, c, _ in per_channel), file=sys.stderr)

    last_meter = 0.0
    try:
        with source:
            while True:
                try:
                    block = source.read()
                except EOFError:
                    break
                except queue.Empty:
                    continue
                values = mix.process(block.mean(axis=1))
                sender.send(values, pulse_keys=mix.pulse_keys)
                for name, ch, analyzer in per_channel:
                    if ch < block.shape[1]:
                        sender.send(analyzer.process(block[:, ch]), scope=f"ch/{name}",
                                    pulse_keys=analyzer.pulse_keys)
                sender.flush()
                if args.meter and time.monotonic() - last_meter > 1 / 15:
                    last_meter = time.monotonic()
                    print("\r" + meter_line(values), end="", file=sys.stderr, flush=True)
    except KeyboardInterrupt:
        pass
    finally:
        sender.flush(force=True)
        if args.meter:
            print(file=sys.stderr)


if __name__ == "__main__":
    main()
