# AV Visual — modo Banda

Cada instrumento de la música tiene **su propia capa visual**, todas a la vez, con estética
minimalista y glitch. No hace falta TouchDesigner ni saber programar.

| Instrumento | Estilos | Dónde |
|---|---|---|
| **Bombo** | Anillos que se expanden · Bloque que golpea · Mira | centro |
| **Caja** | Cortes horizontales · Rayos · Píxeles | toda la pantalla |
| **Hi-hat** | Chispas en cuadrícula · Estática · Glifos | toda la pantalla |
| **Bajo** | Onda · Barras · Vúmetro | abajo |
| **Melodía** | Línea · Escalera · Puntos (la forma de la melodía, de izquierda a derecha) | medio |
| **Armonía** | Teclas (12 notas) · Orbes · Acorde (une las notas que suenan) | arriba / alrededor |

Encima va el **glitch**: separación RGB, bloques desplazados, pixelado, líneas de TV e inversión.
Se controla con *siempre* (permanente), *con golpes* (caja y bombo), *caos* y *líneas de TV*.

Cada capa se enciende o apaga, y tiene color, estilo, brillo, tamaño y **sensibilidad** (en bombo,
caja y hi-hat cambia el umbral de detección; en las otras, la ganancia). Hay 4 paletas (Neón,
Monocromo, Error, Ácido), una leyenda en pantalla y presets guardables (JSON en `presets/`).

**Entradas**: lo que suena en la PC (Spotify, YouTube… en Windows, por defecto), micrófono o
interfaz, canción de demostración, o arrastrar un archivo WAV/MP3/FLAC a la ventana.
Puede mandar los datos a TouchDesigner por OSC al mismo tiempo.

Atajos: `H` panel · `F`/`F11` pantalla completa · `1`–`6` capas · `G` glitch.

## Compilar

```bash
cd rust
cargo run --release -p av-visual                       # abrir la app
cargo run --release -p av-visual -- --demo --sin-panel # demo a pantalla limpia
```

Opciones: `--demo`, `--sin-panel`, `--preset NOMBRE` (de la carpeta `presets/`).

Windows desde Linux:

```bash
rustup target add x86_64-pc-windows-gnu
sudo apt install gcc-mingw-w64-x86-64
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc \
  cargo build --release --target x86_64-pc-windows-gnu -p av-visual
```

En Linux necesita `libasound2-dev` para compilar y `libxkbcommon-x11-0` para abrir la ventana.

## Código

```
src/main.rs      ventana, panel, leyenda, atajos, presets
src/band.rs      capas, paletas, presets y estado (edad de golpes, historia de la melodía, glitch)
src/visuals.rs   shader: las 6 capas + glitch
src/engine.rs    hilo de audio (análisis + OSC opcional)
src/demo.rs      canción de demostración generada por código
```

Para agregar un estilo: suma su nombre en `LAYERS` (`band.rs`) y su dibujo en la capa
correspondiente de `scene()` en `visuals.rs` (usa `u_style[i]`).
