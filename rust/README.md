# av-live (versión en Rust)

El mismo visualizador en vivo que `live.py`, pero como **un solo ejecutable**:
no necesita Python ni instalar dependencias, tiene latencia estable (sin recolector de basura)
y el audio pasa por un buffer circular sin bloqueos.

Envía **exactamente las mismas direcciones OSC** (`/drums/kick`, `/lead/note`, `/tempo/bpm`…),
así que la parte de TouchDesigner no cambia.

## Compilar

Necesitas [Rust](https://rustup.rs) (1.85 o más nuevo).

```bash
cd rust
cargo build --release
# el ejecutable queda en rust/target/release/av-live  (av-live.exe en Windows)
```

En Linux además: `sudo apt install libasound2-dev pkg-config`.
En Windows y Mac no hace falta nada más.

## Usar

```bash
av-live --list-devices        # ver entradas de audio
av-live --meter               # entrada por defecto + medidores en la terminal
av-live --device 3 --meter    # elegir dispositivo (número o parte del nombre)
av-live --sistema --meter     # lo que suena en la PC: Spotify, YouTube… (Windows)
av-live --file tema.mp3       # probar con un archivo (WAV, MP3 o FLAC) como si fuera en vivo
av-live --host 192.168.0.20   # mandar OSC a otra computadora
av-live --help                # todas las opciones
```

## Configuración

El `config.yaml` del proyecto viene **incluido dentro del ejecutable**. Para cambiar algo,
crea un `config.yaml` en la carpeta desde donde lo ejecutas (o usa `--config archivo.yaml`)
con sólo lo que quieras cambiar. Por ejemplo, el modo multicanal:

```yaml
audio:
  inputs:
    - {channel: 1, name: guitarra}
    - {channel: 2, name: voz}
```

A diferencia de la versión en Python, en vivo usa la frecuencia de muestreo nativa del dispositivo
(normalmente 48 kHz) en vez de forzar `audio.sample_rate`, para evitar errores de configuración.

## Comparación con Python

Con la misma canción de prueba, las dos versiones dan **los mismos valores** en todos los bloques
(mismos golpes de bombo/caja/hi-hat, mismas notas y BPM). En la máquina de pruebas:

| | Python | Rust |
|---|---|---|
| tiempo de análisis por bloque (11.6 ms de audio) | ~2.0 ms | ~0.75 ms |
| instalación | Python + 5 paquetes | un archivo de ~3 MB |
| pausas impredecibles | posibles (GC, intérprete) | no |

## Código

```
src/analyzer.rs   análisis en tiempo real (sin reservar memoria por bloque)
src/dsp.rs        suavizado, auto-ganancia, detector de golpes
src/input.rs      entrada en vivo (cpal) y archivos (symphonia)
src/osc.rs        envío OSC
src/config.rs     configuración (mismo config.yaml)
src/main.rs       línea de comandos
tests/            mismas pruebas que la versión en Python + prueba de punta a punta por OSC
examples/dump_features.rs   exporta todos los valores de un archivo a CSV
```

```bash
cargo test --release
cargo run --release --example dump_features -- tema.wav > valores.csv
```
