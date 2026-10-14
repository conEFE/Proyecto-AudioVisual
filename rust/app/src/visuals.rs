//! Dibujo del modo Banda con un shader OpenGL: una capa por instrumento + glitch encima.

use std::sync::{Arc, Mutex};

use eframe::egui;
use eframe::egui_glow;
use eframe::glow::{self, HasContext};

use crate::band::BandUniforms;

const VERTEX: &str = r#"
#if NEW_SHADER_INTERFACE
#define ATTR in
#define VARY out
#else
#define ATTR attribute
#define VARY varying
#endif
ATTR vec2 a_pos;
VARY vec2 v_uv;
void main() {
    v_uv = a_pos * 0.5 + 0.5;
    gl_Position = vec4(a_pos, 0.0, 1.0);
}
"#;

const FRAGMENT: &str = r#"
#ifdef GL_ES
precision highp float;
#endif
#if NEW_SHADER_INTERFACE
in vec2 v_uv;
out vec4 out_color;
#define FRAG out_color
#else
varying vec2 v_uv;
#define FRAG gl_FragColor
#endif
uniform vec2 u_res;
uniform float u_time;
uniform float u_seed;
uniform float u_on[6];
uniform float u_style[6];
uniform vec3 u_col[6];
uniform float u_int[6];
uniform float u_size[6];
uniform float u_kick_age[4];
uniform float u_snare_age;
uniform float u_snare_seed;
uniform float u_hat_age;
uniform float u_hat_seed;
uniform float u_hat_lvl;
uniform float u_bass;
uniform float u_bands[6];
uniform float u_lead;
uniform float u_lead_y[48];
uniform float u_lead_a[48];
uniform float u_chroma[12];
uniform float u_glitch;
uniform float u_chaos;
uniform float u_scan;
uniform float u_grid;

float px;      // tamaño de un pixel en coordenadas de la escena
float aspect;

float h11(float x) { return fract(sin(x * 127.1 + 0.31) * 43758.5453); }
float h21(vec2 p) { return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453); }
float box(vec2 p, vec2 b) { vec2 d = abs(p) - b; return length(max(d, 0.0)) + min(max(d.x, d.y), 0.0); }
float seg(vec2 p, vec2 a, vec2 b) {
    vec2 pa = p - a, ba = b - a;
    return length(pa - ba * clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0));
}

// p: x en [-aspect, aspect], y en [-1, 1]
vec3 scene(vec2 p) {
    vec3 c = vec3(0.0);
    vec2 center = vec2(0.0, 0.05);

    // rejilla de fondo (mínima)
    if (u_grid > 0.5) {
        vec2 g = abs(fract(p * 4.0) - 0.5);
        c += vec3(0.07) * step(0.5 - px * 1.5, max(g.x, g.y));
    }

    // BAJO: abajo
    if (u_on[3] > 0.5) {
        vec3 col = u_col[3] * u_int[3];
        if (u_style[3] < 0.5) {
            float y = -0.72 + sin(p.x * (1.2 + u_size[3] * 1.5) + u_time * 1.3) * (0.02 + 0.22 * u_bass);
            float th = px * 2.0 + 0.02 * u_bass * u_size[3];
            c += col * step(abs(p.y - y), th);
            c += col * 0.16 * u_bass * step(p.y, y);
        } else if (u_style[3] < 1.5) {
            float n = 24.0;
            float xn = p.x / aspect * 0.5 + 0.5;
            float i = floor(xn * n);
            float cell = fract(xn * n);
            float e = u_bass * (0.55 + 0.45 * sin(i * 1.7 + u_time * 2.0));
            float h = 0.03 + 0.45 * e * u_size[3];
            c += col * step(p.y, -1.0 + h) * step(0.15, cell) * step(cell, 0.85);
        } else {
            // vúmetro: segmentos desde el centro hacia los lados
            float sw = 0.07;
            float xi = floor(abs(p.x) / sw);
            float cx = fract(abs(p.x) / sw);
            float lit = step(xi * sw, u_bass * aspect * (0.6 + 0.4 * u_size[3]));
            float row = step(abs(p.y + 0.86), 0.03 * (0.6 + u_size[3]));
            c += col * row * step(0.18, cx) * (lit + 0.12);
        }
    }

    // ARMONÍA: arriba (teclas) o alrededor del centro (orbes)
    if (u_on[5] > 0.5) {
        vec3 col = u_col[5] * u_int[5];
        for (int i = 0; i < 12; i++) {
            float fi = float(i);
            float e = u_chroma[i];
            if (u_style[5] < 0.5) {
                float w = 0.075 * (0.6 + 0.4 * u_size[5]);
                vec2 cp = vec2((fi - 5.5) * w * 1.3, 0.86);
                float d = box(p - cp, vec2(w * 0.5));
                c += col * (step(d, 0.0) * e * e + step(abs(d), px) * 0.3);
            } else if (u_style[5] < 1.5) {
                float ang = fi / 12.0 * 6.2831 - 1.5708;
                vec2 cp = center + vec2(cos(ang), sin(ang)) * (0.5 + 0.2 * u_size[5]);
                float d = length(p - cp) - (0.012 + 0.05 * e * e);
                c += col * (step(d, 0.0) * (0.25 + 0.75 * e) + step(abs(d), px) * 0.3);
            } else {
                // acorde: las notas que suenan, unidas, dibujan la forma del acorde
                float R = 0.5 + 0.2 * u_size[5];
                float ang = fi / 12.0 * 6.2831 - 1.5708;
                vec2 a = center + vec2(cos(ang), sin(ang)) * R;
                c += col * step(length(p - a), 0.006) * 0.5;
                if (e > 0.55) {
                    c += col * step(length(p - a), 0.02);
                    for (int j = 0; j < 12; j++) {
                        if (j > i && u_chroma[j] > 0.55) {
                            float angj = float(j) / 12.0 * 6.2831 - 1.5708;
                            vec2 b = center + vec2(cos(angj), sin(angj)) * R;
                            c += col * step(seg(p, a, b), px * 1.5) * 0.8;
                        }
                    }
                }
            }
        }
    }

    // BOMBO: centro
    if (u_on[0] > 0.5) {
        vec3 col = u_col[0] * u_int[0];
        if (u_style[0] < 0.5) {
            for (int i = 0; i < 4; i++) {
                float a = u_kick_age[i];
                if (a < 1.5) {
                    float r = a * (0.8 + 0.6 * u_size[0]) * 1.4;
                    float d = abs(length(p - center) - r);
                    c += col * step(d, px * 2.0 + 0.012 * (1.0 - a)) * (1.0 - a / 1.5);
                }
            }
            c += col * step(length(p - center), 0.01 + 0.03 * exp(-u_kick_age[0] * 8.0));
        } else if (u_style[0] < 1.5) {
            float a = u_kick_age[0];
            float s = (0.06 + 0.18 * u_size[0]) * (1.0 + 1.2 * exp(-a * 9.0));
            float d = box(p - center, vec2(s));
            c += col * step(d, 0.0) * (0.15 + 0.85 * exp(-a * 7.0));
            c += col * step(abs(d), px * 1.5);
        } else {
            // mira: una cruz que se abre en cada golpe
            float f = exp(-u_kick_age[0] * 7.0);
            vec2 q = p - center;
            float th = px * (1.0 + 3.0 * f);
            float len = 0.12 + (0.4 + 1.6 * f) * u_size[0];
            float arms = step(abs(q.x), len) * step(abs(q.y), th) + step(abs(q.y), len) * step(abs(q.x), th);
            float gap = step(0.03, max(abs(q.x), abs(q.y)));
            float frame = step(abs(box(q, vec2(0.05 + 0.08 * f))), px);
            c += col * (min(arms, 1.0) * gap * (0.25 + 0.75 * f) + frame * (0.3 + 0.7 * f));
        }
    }

    // MELODÍA: la forma de la melodía, de izquierda (antes) a derecha (ahora)
    if (u_on[4] > 0.5) {
        vec3 col = u_col[4] * u_int[4];
        float fidx = (p.x / aspect * 0.5 + 0.5) * 47.0;
        float yv = 0.0, av = 0.0;
        for (int i = 0; i < 48; i++) {
            float w = step(abs(fidx - float(i)), 0.5);
            yv += w * u_lead_y[i];
            av += w * u_lead_a[i];
        }
        float y = mix(-0.4, 0.68, yv);
        float on = step(0.05, av);
        if (u_style[4] < 0.5) {
            c += col * on * step(abs(p.y - y), px * (1.5 + 5.0 * av * u_size[4]));
        } else if (u_style[4] < 1.5) {
            float cx = fract(fidx + 0.5);
            c += col * on * step(abs(p.y - y), 0.012 * u_size[4] + px * 2.0) * step(0.08, cx) * step(cx, 0.92);
        } else {
            // puntos: una marca por instante, más grande cuanto más fuerte
            for (int i = 0; i < 48; i++) {
                float a = u_lead_a[i];
                vec2 q = vec2((float(i) / 47.0 * 2.0 - 1.0) * aspect, mix(-0.4, 0.68, u_lead_y[i]));
                c += col * step(0.05, a) * step(box(p - q, vec2(0.004 + 0.016 * a * u_size[4])), 0.0);
            }
        }
        vec2 head = vec2(aspect * 0.985, mix(-0.4, 0.68, u_lead_y[47]));
        c += col * step(0.05, u_lead_a[47]) * step(box(p - head, vec2(0.008 + 0.02 * u_lead)), 0.0);
    }

    // HI-HAT: chispas por toda la pantalla
    if (u_on[2] > 0.5) {
        vec3 col = u_col[2] * u_int[2];
        float fade = exp(-u_hat_age * 9.0);
        if (u_style[2] < 0.5) {
            float cells = 28.0 / (0.5 + 0.5 * u_size[2]);
            vec2 g = floor(p * cells);
            vec2 f = fract(p * cells);
            float lit = step(1.0 - 0.025 * (0.5 + u_hat_lvl), h21(g + u_hat_seed * 13.1));
            float sq = step(abs(f.x - 0.5), 0.2) * step(abs(f.y - 0.5), 0.2);
            c += col * lit * sq * fade * 1.4;
        } else if (u_style[2] < 1.5) {
            vec2 g = floor(p / (px * (2.0 + 3.0 * u_size[2])));
            float r = h21(g + floor(u_time * 60.0));
            c += col * step(0.94, r) * (fade + 0.06 * u_hat_lvl);
        } else {
            // glifos: caracteres inventados de 3x5 pixeles que aparecen con cada golpe
            float cs = 0.05 * (0.6 + 0.6 * u_size[2]);
            vec2 g = floor(p / cs);
            vec2 fc = (fract(p / cs) - 0.15) / 0.7;
            float inside = step(0.0, fc.x) * step(fc.x, 0.999) * step(0.0, fc.y) * step(fc.y, 0.999);
            vec2 sub = floor(fc * vec2(3.0, 5.0));
            float on = step(0.95, h21(g + u_hat_seed * 5.1));
            float bit = step(0.45, h21(g * 1.7 + sub * 3.1 + u_hat_seed));
            c += col * on * bit * inside * fade * 1.3;
        }
    }

    // CAJA: cortes horizontales o rayos
    if (u_on[1] > 0.5) {
        vec3 col = u_col[1] * u_int[1];
        float fade = exp(-u_snare_age * 9.0);
        if (u_style[1] < 0.5) {
            for (int i = 0; i < 5; i++) {
                float fi = float(i);
                float y = h11(u_snare_seed * 7.3 + fi) * 2.0 - 1.0;
                float h = (0.003 + 0.03 * h11(u_snare_seed + fi * 3.1)) * (0.5 + u_size[1]);
                float x0 = (h11(u_snare_seed * 2.7 + fi) * 2.0 - 1.0) * aspect;
                c += col * step(abs(p.y - y), h) * step(x0 - 0.2, p.x + 0.8 * h11(fi + u_snare_seed)) * fade;
            }
        } else if (u_style[1] < 1.5) {
            for (int i = 0; i < 3; i++) {
                float fi = float(i);
                float ang = h11(u_snare_seed + fi * 1.9) * 3.1416;
                vec2 n = vec2(-sin(ang), cos(ang));
                float d = abs(dot(p, n) - (h11(u_snare_seed * 2.1 + fi) - 0.5) * 1.2);
                c += col * step(d, 0.003 + 0.012 * u_size[1] * (1.0 - fi * 0.3)) * fade * 1.4;
            }
        } else {
            // píxeles: bloques grandes que estallan en cada golpe
            float cs = 0.07 * (0.5 + u_size[1]);
            vec2 g = floor(p / cs);
            float r = h21(g + u_snare_seed * 3.7);
            vec2 f = fract(p / cs);
            float blk = step(0.06, f.x) * step(0.06, f.y);
            c += col * step(0.93 - 0.05 * fade, r) * blk * fade;
        }
    }
    return c;
}

void main() {
    vec2 frag = v_uv * u_res;
    aspect = u_res.x / max(u_res.y, 1.0);
    px = 2.0 / max(u_res.y, 1.0);
    float g = u_glitch;
    vec2 uv = v_uv;

    // pixelado (caos)
    float pix = 1.0 + floor(u_chaos * g * 8.0);
    uv = (floor(uv * u_res / pix) + 0.5) * pix / u_res;

    // bloques desplazados
    float rows = 6.0 + floor(26.0 * h11(u_seed));
    float row = floor(uv.y * rows);
    float r = h11(row * 1.37 + u_seed * 9.1);
    uv.x += step(1.0 - g * (0.35 + 0.5 * u_chaos), r) * (h11(row + u_seed * 3.7) - 0.5) * 0.35 * g;

    vec2 P = (uv * 2.0 - 1.0) * vec2(aspect, 1.0);
    vec3 col;
    float split = g * 0.02 * (0.4 + u_chaos);
    if (split > 0.0008) {
        col.r = scene(P + vec2(split, 0.0)).r;
        col.g = scene(P).g;
        col.b = scene(P - vec2(split, 0.0)).b;
    } else {
        col = scene(P);
    }

    // líneas de TV
    col *= 1.0 - u_scan * 0.45 * step(0.5, fract(frag.y * 0.5));
    // inversión en golpes fuertes (caos)
    float inv = step(0.75, g) * step(0.35, u_chaos) * step(0.55, h11(u_seed * 3.3));
    col = mix(col, vec3(1.0) - col, inv);
    FRAG = vec4(clamp(col, 0.0, 1.0), 1.0);
}
"#;

pub struct Renderer {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
}

impl Renderer {
    pub fn new(gl: &glow::Context) -> Result<Self, String> {
        let version = egui_glow::ShaderVersion::get(gl);
        let header = format!(
            "{}#define NEW_SHADER_INTERFACE {}\n",
            version.version_declaration(),
            i32::from(version.is_new_shader_interface())
        );
        unsafe {
            let program = gl.create_program()?;
            let mut shaders = Vec::new();
            for (kind, src) in [(glow::VERTEX_SHADER, VERTEX), (glow::FRAGMENT_SHADER, FRAGMENT)] {
                let sh = gl.create_shader(kind)?;
                gl.shader_source(sh, &format!("{header}{src}"));
                gl.compile_shader(sh);
                if !gl.get_shader_compile_status(sh) {
                    return Err(format!("error en el shader: {}", gl.get_shader_info_log(sh)));
                }
                gl.attach_shader(program, sh);
                shaders.push(sh);
            }
            gl.bind_attrib_location(program, 0, "a_pos");
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(format!(
                    "error al enlazar el shader: {}",
                    gl.get_program_info_log(program)
                ));
            }
            for sh in shaders {
                gl.detach_shader(program, sh);
                gl.delete_shader(sh);
            }
            // un triángulo que cubre toda el área
            let verts: [f32; 6] = [-1.0, -1.0, 3.0, -1.0, -1.0, 3.0];
            let bytes: Vec<u8> = verts.iter().flat_map(|v| v.to_le_bytes()).collect();
            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, &bytes, glow::STATIC_DRAW);
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
            gl.bind_vertex_array(None);
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            Ok(Self { program, vao, vbo })
        }
    }

    fn paint(&self, gl: &glow::Context, u: &BandUniforms, size: [f32; 2]) {
        unsafe {
            gl.use_program(Some(self.program));
            let p = self.program;
            // los arreglos se buscan como "nombre" o "nombre[0]" según el driver
            let loc = |name: &str| {
                gl.get_uniform_location(p, name)
                    .or_else(|| gl.get_uniform_location(p, &format!("{name}[0]")))
            };
            let f1 = |name: &str, v: f32| gl.uniform_1_f32(loc(name).as_ref(), v);
            let fv = |name: &str, v: &[f32]| gl.uniform_1_f32_slice(loc(name).as_ref(), v);
            gl.uniform_2_f32(loc("u_res").as_ref(), size[0], size[1]);
            f1("u_time", u.time);
            f1("u_seed", u.seed);
            fv("u_on", &u.on);
            fv("u_style", &u.style);
            let cols: Vec<f32> = u.color.iter().flatten().copied().collect();
            gl.uniform_3_f32_slice(loc("u_col").as_ref(), &cols);
            fv("u_int", &u.intensity);
            fv("u_size", &u.size);
            fv("u_kick_age", &u.kick_age);
            f1("u_snare_age", u.snare_age);
            f1("u_snare_seed", u.snare_seed);
            f1("u_hat_age", u.hat_age);
            f1("u_hat_seed", u.hat_seed);
            f1("u_hat_lvl", u.hat_level);
            f1("u_bass", u.bass);
            fv("u_bands", &u.bands);
            f1("u_lead", u.lead);
            fv("u_lead_y", &u.lead_y);
            fv("u_lead_a", &u.lead_a);
            fv("u_chroma", &u.chroma);
            f1("u_glitch", u.glitch);
            f1("u_chaos", u.chaos);
            f1("u_scan", u.scanlines);
            f1("u_grid", u.grid);
            gl.disable(glow::BLEND);
            gl.bind_vertex_array(Some(self.vao));
            gl.draw_arrays(glow::TRIANGLES, 0, 3);
            gl.bind_vertex_array(None);
            gl.enable(glow::BLEND);
        }
    }

    pub fn destroy(&self, gl: &glow::Context) {
        unsafe {
            gl.delete_program(self.program);
            gl.delete_vertex_array(self.vao);
            gl.delete_buffer(self.vbo);
        }
    }
}

/// Agrega el dibujo del visual en `rect`.
pub fn paint(ui: &egui::Ui, rect: egui::Rect, renderer: &Arc<Mutex<Renderer>>, u: BandUniforms) {
    let renderer = renderer.clone();
    let cb = egui_glow::CallbackFn::new(move |info, painter| {
        let vp = info.viewport_in_pixels();
        renderer
            .lock()
            .unwrap()
            .paint(painter.gl(), &u, [vp.width_px as f32, vp.height_px as f32]);
    });
    ui.painter().add(egui::PaintCallback {
        rect,
        callback: Arc::new(cb),
    });
}
