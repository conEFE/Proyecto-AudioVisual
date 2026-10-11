import threading

import numpy as np
import soundfile as sf
from pythonosc.dispatcher import Dispatcher
from pythonosc.osc_server import ThreadingOSCUDPServer

from visualizer.analyzer import LiveAnalyzer
from visualizer.audio_io import FileInput
from visualizer.config import load_config
from visualizer.osc_out import OscSender


def test_file_to_osc(tmp_path):
    sr = 44100
    t = np.arange(sr * 2) / sr
    sig = 0.3 * np.sin(2 * np.pi * 220 * t)
    sig[:: sr // 2] += 0.9  # clics
    path = tmp_path / "test.wav"
    sf.write(path, np.stack([sig, sig], axis=1), sr)

    received = {}
    d = Dispatcher()
    d.set_default_handler(lambda addr, *v: received.setdefault(addr, []).append(v[0]))
    server = ThreadingOSCUDPServer(("127.0.0.1", 0), d)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    port = server.server_address[1]

    cfg = load_config()
    an = LiveAnalyzer(sr, cfg)
    sender = OscSender("127.0.0.1", port, prefix="av", send_fps=0)
    with FileInput(str(path), sr, cfg["analysis"]["hop_size"], playback=False, realtime=False) as src:
        while True:
            try:
                block = src.read()
            except EOFError:
                break
            sender.send(an.process(block.mean(axis=1)), pulse_keys=an.pulse_keys)
            sender.flush()
    import time
    time.sleep(0.3)
    server.shutdown()

    for addr in ["/av/mix/level", "/av/drums/kick", "/av/band/bass", "/av/tempo/bpm", "/av/lead/note"]:
        assert addr in received, addr
    assert abs(np.median(received["/av/lead/pitch_hz"][-20:]) - 220) < 10
