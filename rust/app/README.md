# AV Visual

Visuales que reaccionan a la música en vivo, con un editor visual: **no hace falta TouchDesigner
ni saber programar**. Usa el mismo análisis de audio que `av-live`.

## Qué tiene

- **5 escenas** dibujadas en la GPU: Nebulosa, Túnel, Caleidoscopio, Espectro y Ondas.
- **Editor "Qué mueve cada cosa"**: para cada parámetro de la imagen (Tamaño, Velocidad, Destello,
  Color, Saturación, Detalle, Distorsión, Giro) eliges qué parte del sonido lo mueve
  (graves, bombo, caja, hi-hat, bajo, melodía, nota, tempo…) y ajustas *base*, *cuánto* y *calma*.
- **Presets**: guarda y carga tus combinaciones (JSON en la carpeta `presets/` junto al ejecutable).
- **Cambio de escena automático** cada N compases, siguiendo el tempo.
- **Entradas**: lo que suena en la PC (Spotify, YouTube… en Windows, sin instalar nada; es la
  entrada por defecto), micrófono / interfaz, canción de demostración incluida, o arrastrar un
  archivo WAV/MP3/FLAC a la ventana.
- **OSC opcional**: puede mandar los mismos datos a TouchDesigner al mismo tiempo.

Atajos: `H` panel · `F`/`F11` pantalla completa · `1`–`5` escena.

## Compilar

```bash
cd rust
cargo run --release -p av-visual            # abrir la app
cargo run --release -p av-visual -- --demo  # con la canción de demostración
```

Opciones: `--demo`, `--escena N`, `--sin-panel`.

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
src/main.rs      ventana, panel de edición, atajos, presets
src/visuals.rs   shader con las 5 escenas
src/mapping.rs   fuentes de sonido, parámetros, presets y suavizado
src/engine.rs    hilo de audio (análisis + OSC opcional)
src/demo.rs      canción de demostración generada por código
```

Para agregar una escena: suma un caso en el shader de `visuals.rs` (usa `u_p[0..7]` y `u_bands`),
su nombre en `SCENES` y su preset inicial en `Preset::default_for` (`mapping.rs`).
