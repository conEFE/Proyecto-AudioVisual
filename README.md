# Proyecto-AudioVisual

Visualizador de música **en vivo**: escucha el audio (micrófono, interfaz de audio o lo que
suena en tu PC), analiza frecuencias e instrumentos en tiempo real y envía los datos por
**OSC** a **TouchDesigner** (o a cualquier programa que reciba OSC: Resolume, Max, Processing…).

```
entrada de audio ──► live.py (análisis ~12 ms) ──OSC──► TouchDesigner ──► visuales
```

Hay dos versiones que hacen lo mismo y envían los mismos datos:

| | Cuándo usarla |
|---|---|
| **Rust** ([`rust/`](rust/README.md)) — `av-live` | para tocar en vivo: un solo ejecutable, latencia estable, ~3× más rápido |
| **Python** — `live.py` | para experimentar y cambiar el algoritmo rápido |

## Instalación (Rust)

```bash
cd rust && cargo build --release      # ejecutable en rust/target/release/av-live
```

Ver [`rust/README.md`](rust/README.md) para los detalles.

## Instalación (Python)

Requiere Python 3.10+.

```bash
pip install -r requirements.txt
```

En Linux además: `sudo apt install libportaudio2`.

## Uso

Con Rust es igual, cambiando `python live.py` por `av-live`.

```bash
python live.py --list-devices        # ver entradas de audio disponibles
python live.py --meter               # entrada por defecto + medidores en la terminal
python live.py --device 3 --meter    # elegir dispositivo
python live.py --file tema.wav       # probar con un archivo como si fuera en vivo
python tools/osc_monitor.py          # (en otra terminal) ver lo que llega por OSC
```

Para TouchDesigner, ver [`touchdesigner/README.md`](touchdesigner/README.md). Incluye un script que arma
una red de ejemplo automáticamente.

### ¿De dónde sale el audio?

| Situación | Cómo |
|---|---|
| Micrófono / línea | el dispositivo por defecto, o `--device N` |
| Banda en vivo con interfaz multicanal | configura `audio.inputs` en `config.yaml` (un instrumento por canal) |
| Lo que suena en el PC (Spotify, DJ, DAW) | **Windows:** "Mezcla estéreo" o [VB-Audio Cable](https://vb-audio.com/Cable/) · **Mac:** [BlackHole](https://github.com/ExistentialAudio/BlackHole) · **Linux:** el "monitor" de PulseAudio/PipeWire |

## Qué envía (direcciones OSC)

Valores normalizados 0..1, salvo donde se indica. Se ajustan solos al volumen (auto-ganancia).

| Dirección | Qué es |
|---|---|
| `/mix/level` | volumen general |
| `/mix/brightness` | brillo del sonido (agudo = 1) |
| `/mix/noisiness` | ruido vs. tono (platillos/distorsión = alto) |
| `/mix/harmonic`, `/mix/percussive` | cuánto es sonido sostenido vs. golpes |
| `/mix/onset` | pulso en cualquier ataque |
| `/band/sub`, `bass`, `lowmid`, `mid`, `highmid`, `high` | energía por banda de frecuencia |
| `/drums/kick`, `/drums/snare`, `/drums/hihat` | **pulso** en cada golpe (salta a 1 y cae) |
| `/drums/*_level` | nivel continuo de cada parte de la batería |
| `/bass` | nivel del bajo (graves sostenidos) |
| `/lead` | nivel de melodía / voz / instrumentos armónicos medios |
| `/lead/pitch_hz`, `/lead/note` (MIDI), `/lead/pitch_class` (0=Do … 11=Si), `/lead/confidence` | nota de la melodía |
| `/chroma/c` … `/chroma/b` | cuánto suena cada una de las 12 notas |
| `/tempo/bpm` | tempo estimado (BPM) |
| `/tempo/phase` | fase del pulso 0→1 (para animar en sincronía) |
| `/tempo/beat` | pulso en cada tiempo |
| `/ch/<nombre>/...` | lo mismo, por cada canal en modo multicanal |

## Cómo detecta los instrumentos

Separar instrumentos con IA (por ejemplo Demucs) no funciona en tiempo real, así que en vivo se usan
técnicas rápidas (cada bloque de ~12 ms se analiza en menos de 1 ms):

1. **Separación armónico/percusiva (HPSS)**: compara cada instante con los anteriores (lo sostenido
   es armónico) y cada frecuencia con sus vecinas (lo que golpea en todo el espectro es percusivo).
2. **Batería**: golpes en la parte percusiva, por zona: bombo 40–120 Hz, caja 150–400 Hz *y* 2–6 kHz
   a la vez, hi-hat 7–16 kHz.
3. **Bajo y melodía**: energía de la parte armónica en graves (bajo) y medios (voz/guitarra/sintes).
4. **Nota**: suma de armónicos sobre la parte armónica.
5. **Tempo**: autocorrelación de los golpes de los últimos segundos y un oscilador que se sincroniza
   con el bombo.

Es una aproximación: dos instrumentos en la misma zona de frecuencias (ej. guitarra y voz) se ven juntos
como `lead`. Para separación exacta en vivo usa el **modo multicanal** (cada músico en su canal).
Todos los rangos y sensibilidades se ajustan en [`config.yaml`](config.yaml).

## Estructura

```
rust/                     versión en Rust (av-live)
live.py                   programa principal en Python (CLI)
config.yaml               bandas, instrumentos, OSC, latencia
visualizer/analyzer.py    análisis en tiempo real
visualizer/dsp.py         suavizado, auto-ganancia, detector de golpes
visualizer/audio_io.py    entrada en vivo y archivo
visualizer/osc_out.py     envío OSC
tools/osc_monitor.py      ver los mensajes OSC sin TouchDesigner
touchdesigner/            guía y red de ejemplo para TouchDesigner
tests/                    pruebas con señales sintéticas
```

## Pruebas

```bash
pip install pytest
python -m pytest
cd rust && cargo test --release
```

## Próximos pasos posibles

- Clasificador de instrumentos con IA en segundo plano (YAMNet/PANNs) para etiquetas como
  "guitarra", "piano", "voz" cada ~0.5 s.
- Presets de mapeo y escenas en TouchDesigner.
- Salida MIDI además de OSC.
