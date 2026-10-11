"""Fuentes de audio: entrada en vivo (micrófono / interfaz / loopback) o archivo simulando vivo."""

import queue
import threading
import time

import numpy as np


def _sd():
    try:
        import sounddevice as sd
    except OSError as e:  # PortAudio no instalado
        raise SystemExit(f"No se pudo cargar sounddevice/PortAudio: {e}") from e
    return sd


def list_devices() -> str:
    return str(_sd().query_devices())


class LiveInput:
    """Captura bloques de `hop` muestras desde un dispositivo de entrada."""

    def __init__(self, sample_rate: int, hop: int, channels: int = 1, device=None):
        self.q = queue.Queue(maxsize=64)
        self.channels = channels
        self.stream = _sd().InputStream(samplerate=sample_rate, blocksize=hop, channels=channels,
                                        device=device, dtype="float32", callback=self._callback)

    def _callback(self, indata, frames, time_info, status):
        try:
            self.q.put_nowait(indata.copy())
        except queue.Full:
            pass  # si el análisis se atrasa, se descartan bloques en vez de acumular latencia

    def __enter__(self):
        self.stream.start()
        return self

    def __exit__(self, *exc):
        self.stream.stop()
        self.stream.close()

    def read(self, timeout=1.0):
        return self.q.get(timeout=timeout)


class FileInput:
    """Reproduce un archivo como si fuera audio en vivo (útil para probar sin micrófono).

    Con `playback=True` suena por los parlantes y el análisis va sincronizado con lo que se oye.
    """

    def __init__(self, path: str, sample_rate: int, hop: int, playback: bool = True,
                 realtime: bool = True, loop: bool = False):
        import soundfile as sf

        data, sr = sf.read(path, dtype="float32", always_2d=True)
        if sr != sample_rate:
            data = _resample(data, sr, sample_rate)
        self.data = data
        self.channels = data.shape[1]
        self.sr, self.hop = sample_rate, hop
        self.playback, self.realtime, self.loop = playback, realtime, loop
        self.q = queue.Queue(maxsize=64)
        self.pos = 0
        self.done = threading.Event()
        self.stream = None
        self.thread = None

    def _next_block(self):
        if self.pos >= len(self.data):
            if not self.loop:
                return None
            self.pos = 0
        block = self.data[self.pos:self.pos + self.hop]
        self.pos += self.hop
        if len(block) < self.hop:
            block = np.pad(block, ((0, self.hop - len(block)), (0, 0)))
        return block

    def _callback(self, outdata, frames, time_info, status):
        block = self._next_block()
        if block is None:
            outdata.fill(0)
            self.done.set()
            raise _sd().CallbackStop
        outdata[:] = block
        try:
            self.q.put_nowait(block.copy())
        except queue.Full:
            pass

    def _pump(self):
        t0 = time.monotonic()
        n = 0
        while True:
            block = self._next_block()
            if block is None:
                break
            self.q.put(block)
            n += 1
            if self.realtime:
                delay = t0 + n * self.hop / self.sr - time.monotonic()
                if delay > 0:
                    time.sleep(delay)
        self.done.set()

    def __enter__(self):
        if self.playback:
            self.stream = _sd().OutputStream(samplerate=self.sr, blocksize=self.hop,
                                             channels=self.channels, dtype="float32",
                                             callback=self._callback)
            self.stream.start()
        else:
            self.thread = threading.Thread(target=self._pump, daemon=True)
            self.thread.start()
        return self

    def __exit__(self, *exc):
        if self.stream:
            self.stream.stop()
            self.stream.close()

    def read(self, timeout=1.0):
        try:
            return self.q.get(timeout=timeout)
        except queue.Empty:
            if self.done.is_set():
                raise EOFError
            raise


def _resample(data: np.ndarray, sr_in: int, sr_out: int) -> np.ndarray:
    n_out = int(round(len(data) * sr_out / sr_in))
    x_in = np.linspace(0, 1, len(data), endpoint=False)
    x_out = np.linspace(0, 1, n_out, endpoint=False)
    return np.stack([np.interp(x_out, x_in, data[:, c]) for c in range(data.shape[1])],
                    axis=1).astype(np.float32)
