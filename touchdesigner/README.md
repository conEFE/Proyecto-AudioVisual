# Integración con TouchDesigner

`live.py` envía todo por **OSC (UDP)**. TouchDesigner lo recibe con un **OSC In CHOP**.

## Opción A: red de ejemplo automática

1. Abre TouchDesigner.
2. Crea un **Text DAT** y pega el contenido de [`build_demo.py`](build_demo.py).
3. Click derecho sobre el DAT → **Run Script**.
4. En tu terminal: `av-live --meter` (Rust) o `python live.py --meter` (Python)
5. Mira el TOP `/project1/audiovisual/salida`.

> El script usa la API estándar de TouchDesigner, pero no se probó dentro de TD en este repo.
> Si algún parámetro cambió de nombre en tu versión, la Opción B hace lo mismo a mano.

## Opción B: a mano

1. Crea un **OSC In CHOP** → `Network Port: 7000`.
2. Pon a sonar algo y ejecuta `python live.py`. Vas a ver aparecer canales como
   `drums/kick`, `band/bass`, `lead/note`, etc.
3. Para usar sólo algunos: **Select CHOP** con `Channel Names: *drums*` (por ejemplo).
4. Para suavizar o exagerar: **Lag CHOP**, **Filter CHOP**, **Math CHOP** (Range 0..1 → lo que quieras).
5. Arrastra un canal sobre un parámetro → **CHOP Reference** (o escribe
   `op('oscin1').chan('*drums/kick')` en el parámetro; el `*` sirve tanto si tu versión
   de TD guarda la `/` inicial en el nombre del canal como si no).

## Ideas de mapeo

| Dato | Sugerencia visual |
|---|---|
| `drums/kick` | escala, destello, explosión de partículas, "flash" de cámara |
| `drums/snare` | cortes, glitch, cambio de contraste |
| `drums/hihat` | detalle fino, partículas pequeñas, ruido de alta frecuencia |
| `bass` / `band/bass` | desplazamiento de geometría, ondas lentas, zoom |
| `lead` | intensidad del color, brillo de la "voz" |
| `lead/pitch_class` | tono de color (0..11 → 0..360°) |
| `chroma/*` | 12 elementos que se encienden según las notas que suenan |
| `tempo/phase` | animaciones en sincronía con el pulso (rotaciones, LFOs) |
| `tempo/beat` | cambios de escena cada pulso |
| `mix/brightness` | paleta fría/cálida |
| `mix/noisiness` | cuánto "grano" o distorsión |
| `ch/<nombre>/*` | (multicanal) un elemento visual por músico |

## Notas

- Los pulsos (`drums/*`, `tempo/beat`, `mix/onset`) saltan a 1 en el golpe y caen solos,
  no hace falta detectar el flanco en TD.
- Si TouchDesigner corre en otra computadora: `python live.py --host 192.168.x.x`.
- Si quieres usar varios programas a la vez, cambia el puerto con `--port`.
