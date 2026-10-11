"""Bloques DSP pequeños y reutilizables: suavizado, auto-ganancia y detección de golpes."""

from collections import deque

import numpy as np


def time_to_coef(seconds: float, fps: float) -> float:
    """Convierte un tiempo de respuesta (s) en coeficiente de suavizado por frame."""
    if seconds <= 0:
        return 0.0
    return float(np.exp(-1.0 / (seconds * fps)))


class Envelope:
    """Seguidor de envolvente con ataque y caída independientes."""

    def __init__(self, attack: float, release: float, fps: float):
        self.a = time_to_coef(attack, fps)
        self.r = time_to_coef(release, fps)
        self.value = 0.0

    def __call__(self, x: float) -> float:
        coef = self.a if x > self.value else self.r
        self.value = coef * self.value + (1.0 - coef) * x
        return self.value


class AutoGain:
    """Normaliza una señal a 0..1 siguiendo su pico reciente.

    Así los valores enviados a TouchDesigner tienen siempre un rango útil,
    sin importar el volumen de entrada.
    """

    def __init__(self, decay_seconds: float, fps: float, floor: float = 1e-6):
        self.decay = time_to_coef(decay_seconds, fps)
        self.floor = floor
        self.peak = floor

    def __call__(self, x: float) -> float:
        self.peak = max(x, self.peak * self.decay, self.floor)
        return float(min(x / self.peak, 1.0))


class OnsetDetector:
    """Detecta golpes en una curva de flujo espectral con umbral adaptativo.

    Devuelve un pulso que salta a 1 en el golpe y cae suavemente, para que el
    receptor no se pierda el evento aunque lea a otra tasa de frames.
    """

    def __init__(self, fps: float, sensitivity: float = 1.5, history_seconds: float = 1.0,
                 refractory_seconds: float = 0.08, pulse_release: float = 0.15,
                 min_flux: float = 1e-4):
        self.history = deque(maxlen=max(4, int(history_seconds * fps)))
        self.sensitivity = sensitivity
        self.refractory = max(1, int(refractory_seconds * fps))
        self.cooldown = 0
        self.min_flux = min_flux
        self.pulse_decay = time_to_coef(pulse_release, fps)
        self.pulse = 0.0
        self.triggered = False

    def __call__(self, flux: float) -> float:
        self.triggered = False
        if len(self.history) >= 4:
            h = np.asarray(self.history)
            threshold = h.mean() + self.sensitivity * h.std()
            if self.cooldown == 0 and flux > max(threshold, self.min_flux):
                self.triggered = True
                self.cooldown = self.refractory
        self.history.append(flux)
        if self.cooldown > 0:
            self.cooldown -= 1
        self.pulse = 1.0 if self.triggered else self.pulse * self.pulse_decay
        return self.pulse
