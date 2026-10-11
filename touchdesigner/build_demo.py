"""Construye una red de ejemplo en TouchDesigner que reacciona al audio en vivo.

Cómo usarlo (dentro de TouchDesigner):
  1. Crea un Text DAT, pega este archivo completo.
  2. Click derecho sobre el DAT → "Run Script" (o Ctrl+R).
  3. Se crea el componente /project1/audiovisual con todo conectado.
  4. Ejecuta `python live.py` y mira el TOP "salida".

Lo que hace la demo:
  * graves (band/bass) → escala y velocidad del ruido
  * bombo  (drums/kick) → destello de brillo
  * hi-hat (drums/hihat) → detalle / rugosidad
  * nota de la melodía (lead/pitch_class) → tono de color (12 notas = círculo de color)
  * caja (drums/snare) → contraste
"""

PORT = 7000

root = op('/project1')
old = root.op('audiovisual')
if old:
    old.destroy()
comp = root.create(baseCOMP, 'audiovisual')


def make(optype, name, x, y):
    o = comp.create(optype, name)
    o.nodeX, o.nodeY = x, y
    return o


def expr(par, text):
    par.expr = text
    par.mode = ParMode.EXPRESSION


osc = make(oscinCHOP, 'av_osc', 0, 200)
osc.par.port = PORT

# Módulo de ayuda: mod.av.v('drums/kick') devuelve 0 si el canal aún no llegó.
helper = make(textDAT, 'av', 0, 0)
helper.text = (
    "def v(name, default=0.0):\n"
    "    c = op('av_osc').chan('*' + name)\n"
    "    return c.eval() if c is not None else default\n"
)

noise = make(noiseTOP, 'ruido', 200, 200)
noise.par.resolutionw = 1280
noise.par.resolutionh = 720
expr(noise.par.period, "1.5 + 3 * mod.av.v('band/bass')")
expr(noise.par.harmon, "2 + int(4 * mod.av.v('drums/hihat'))")
expr(noise.par.tz, "absTime.seconds * (0.1 + 0.6 * mod.av.v('mix/level'))")

level = make(levelTOP, 'golpe', 400, 200)
level.inputConnectors[0].connect(noise)
expr(level.par.brightness1, "0.8 + 1.2 * mod.av.v('drums/kick')")
expr(level.par.contrast, "1 + 0.8 * mod.av.v('drums/snare')")

hsv = make(hsvadjustTOP, 'color', 600, 200)
hsv.inputConnectors[0].connect(level)
expr(hsv.par.hueoffset, "mod.av.v('lead/pitch_class') / 12.0 * 360")
expr(hsv.par.saturationmult, "1 + 2 * mod.av.v('lead')")

out = make(nullTOP, 'salida', 800, 200)
out.inputConnectors[0].connect(hsv)
out.viewer = True

print('Red "audiovisual" creada. Escuchando OSC en el puerto', PORT)
