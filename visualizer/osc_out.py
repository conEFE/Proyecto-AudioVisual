"""Envío de los datos de análisis por OSC (TouchDesigner, Resolume, Max, etc.)."""

import time

from pythonosc.udp_client import SimpleUDPClient


class OscSender:
    def __init__(self, host: str, port: int, prefix: str = "", send_fps: float = 60):
        self.client = SimpleUDPClient(host, port)
        self.prefix = "/" + prefix.strip("/") if prefix.strip("/") else ""
        self.interval = 1.0 / send_fps if send_fps else 0.0
        self.last = 0.0
        self.pending = {}

    def send(self, values: dict, scope: str = "", pulse_keys=()):
        """Acumula valores y los envía respetando `send_fps`.

        Los pulsos (golpes) se guardan como el máximo desde el último envío,
        así un golpe corto nunca se pierde aunque se envíe a menos fps.
        """
        base = self.prefix + ("/" + scope.strip("/") if scope else "")
        for key, value in values.items():
            addr = f"{base}/{key}"
            self.pending[addr] = max(self.pending.get(addr, value), value) if key in pulse_keys else value

    def flush(self, force: bool = False) -> bool:
        now = time.monotonic()
        if not force and now - self.last < self.interval:
            return False
        for addr, value in self.pending.items():
            self.client.send_message(addr, float(value))
        self.pending.clear()
        self.last = now
        return True

