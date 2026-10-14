# Banco de pruebas de detección

Mide **qué tan bien detecta el analizador cada instrumento** y calibra los detectores de batería.

## Cómo funciona

1. `make_songs.py` genera canciones con instrumentos reales (MIDI + soundfont General MIDI) y su
   "respuesta correcta": el instante exacto de cada golpe de bombo, caja y hi-hat, y las notas del
   bajo, la melodía y los acordes. Varía estilo (rock, house, hip hop, funk, balada, drum & bass),
   tempo (70–174 BPM), instrumento de la melodía, mezcla y batería (6 baterías del soundfont o
   baterías **sintetizadas al azar**: 808, acústicas, electrónicas, con palmas…). Hay canciones
   con breaks de batería sola y canciones de **golpes sueltos** en silencio.
2. `train_drums.py` calibra los detectores: aprende cuánto pesa cada una de 15 zonas de frecuencia
   (cuánto sube el sonido y cuánto se sostiene) para cada instrumento, y escribe `drum_model.json`,
   que usan la versión en Rust y la de Python.
3. `evaluate.py` compara lo detectado con la respuesta correcta: precisión (cuántos detectados eran
   reales), recall (cuántos reales se detectaron), F1 y latencia.

```bash
sudo apt install fluidsynth fluid-soundfont-gm
pip install mido scikit-learn soundfile numpy

python tools/bench/make_songs.py bench/train 30 2000
python tools/bench/make_songs.py bench/strain 30 3000 --sintetizada
python tools/bench/make_songs.py bench/solo 12 5000 --sueltos
python tools/bench/make_songs.py bench/ssolo 12 6000 --sueltos --sintetizada
python tools/bench/make_songs.py bench/test 12 1000          # para probar (otras semillas)
python tools/bench/make_songs.py bench/stest 12 4000 --sintetizada

python tools/bench/train_drums.py --entrenar bench/train bench/strain bench/solo bench/ssolo \
                                  --probar bench/test bench/stest
(cd rust && cargo build --release --example dump_features)
python tools/bench/evaluate.py bench/test             # analizador en Rust
python tools/bench/evaluate.py bench/test --python    # analizador en Python
```

## Resultados

F1 (1 = perfecto) en canciones que la calibración **no vio**:

| | Antes | Ahora | Latencia |
|---|---|---|---|
| Bombo | 0,88 | 0,94–0,98 | ~24 ms |
| Caja | 0,63 | 0,84–0,90 | ~39 ms |
| Hi-hat | 0,77 | 0,92–0,93 (0,77–0,82 con golpes sueltos en silencio) | ~13 ms |
| Nota de la melodía | 48 % | ~59 % del tiempo | |

Qué cambió:

- **Batería**: antes cada golpe se decidía con una o dos zonas de frecuencia y umbrales a mano; la
  caja se confundía con "bombo + hi-hat" sonando juntos. Ahora cada detector mira 15 zonas, el
  flujo relativo a lo que viene sonando y cuánto se sostiene el sonido. La caja espera ~38 ms antes
  de decidir: el "chasquido" de la caja dura, el del bombo y el hi-hat cerrado no.
- **Melodía**: suma de 8 armónicos sobre el espectro completo entre 150 y 1500 Hz, nota afinada con
  la posición exacta de cada armónico, y se resta el "fondo" de ~1 s (los acordes que vienen
  sonando) para que resalte lo que cambia.
- **Frecuencia de muestreo**: el tamaño de bloque se escala para que dure lo mismo a 44,1 o 48 kHz.

Límites conocidos: los sonidos son de un soundfont y de síntesis, no de grabaciones reales; con
música real (mezclas comprimidas, reverb, distorsión) los números pueden ser menores. Un bombo
808 muy agudo a veces se confunde con caja. La melodía falla sobre todo cuando un acorde suena más
fuerte que ella o justo en los cambios de nota.
