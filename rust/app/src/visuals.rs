//! Dibujo de los visuales con un shader OpenGL (5 escenas en un solo programa).

use std::sync::{Arc, Mutex};

use eframe::egui;
use eframe::egui_glow;
use eframe::glow::{self, HasContext};

/// Lo que el shader necesita en cada cuadro.
#[derive(Clone, Copy, Debug, Default)]
pub struct Uniforms {
    pub time: f32,
    pub rot: f32,
    pub params: [f32; 8],
    pub bands: [f32; 6],
    pub scene: f32,
}

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
uniform float u_rot;
uniform float u_p[8];
uniform float u_bands[6];
uniform float u_scene;

float hash(vec2 p) {
    p = fract(p * vec2(123.34, 456.21));
    p += dot(p, p + 45.32);
    return fract(p.x * p.y);
}
float noise(vec2 p) {
    vec2 i = floor(p), f = fract(p);
    vec2 u = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash(i), hash(i + vec2(1.0, 0.0)), u.x),
               mix(hash(i + vec2(0.0, 1.0)), hash(i + vec2(1.0, 1.0)), u.x), u.y);
}
float fbm(vec2 p, float oct) {
    float v = 0.0, a = 0.5;
    for (int i = 0; i < 8; i++) {
        if (float(i) >= oct) break;
        v += a * noise(p);
        p = p * 2.03 + vec2(1.7, 9.2);
        a *= 0.5;
    }
    return v;
}
vec3 hsv(float h, float s, float v) {
    vec3 k = clamp(abs(mod(h * 6.0 + vec3(0.0, 4.0, 2.0), 6.0) - 3.0) - 1.0, 0.0, 1.0);
    return v * mix(vec3(1.0), k, s);
}
mat2 rot(float a) { float c = cos(a), s = sin(a); return mat2(c, -s, s, c); }
// energía del espectro en x = 0 (graves) .. 1 (agudos), interpolando las 6 bandas
float bandAt(float x) {
    float f = clamp(x, 0.0, 1.0) * 5.0;
    float e = 0.0;
    for (int k = 0; k < 6; k++) e += max(0.0, 1.0 - abs(f - float(k))) * u_bands[k];
    return e;
}

void main() {
    vec2 uv = v_uv * 2.0 - 1.0;
    uv.x *= u_res.x / max(u_res.y, 1.0);
    float zoom = u_p[0], flash = u_p[2], hue = u_p[3];
    float sat = clamp(u_p[4], 0.0, 1.0), detail = clamp(u_p[5], 0.0, 1.5), dist = u_p[6];
    float t = u_time;
    vec2 p = rot(u_rot) * uv / (0.6 + 1.4 * zoom);
    vec3 col;
    if (u_scene < 0.5) {            // Nebulosa
        vec2 q = vec2(fbm(p * 1.5 + vec2(0.0, t * 0.3), 4.0), fbm(p * 1.5 + vec2(5.2, 1.3) - t * 0.2, 4.0));
        float n = fbm(p * 2.0 + q * (1.0 + 3.0 * dist) + t * 0.1, 3.0 + 4.0 * detail);
        col = hsv(hue + n * 0.25, sat, smoothstep(0.15, 0.95, n) * (0.6 + flash * 1.4));
    } else if (u_scene < 1.5) {     // Túnel
        float r = length(p), a = atan(p.y, p.x);
        float z = 0.35 / max(r, 0.02) + t * 1.5;
        float rings = 0.5 + 0.5 * sin(z * 6.2831 * (1.0 + detail * 2.0));
        float spokes = 0.5 + 0.5 * sin(a * floor(3.0 + detail * 10.0) + z * dist * 3.0);
        float v = mix(rings, rings * spokes, 0.5 + 0.5 * clamp(dist, 0.0, 1.0)) * smoothstep(0.0, 0.35, r);
        col = hsv(hue + z * 0.02, sat, v * (0.5 + flash * 1.5));
        col += flash * 0.3 * exp(-r * 3.0);
    } else if (u_scene < 2.5) {     // Caleidoscopio
        float seg = floor(3.0 + detail * 9.0);
        float r = length(p), a = atan(p.y, p.x);
        float s = 6.2831 / seg;
        a = abs(mod(a, s) - s * 0.5);
        vec2 k = vec2(cos(a), sin(a)) * r;
        float n = fbm(k * 3.0 + vec2(t * 0.4, -t * 0.2) + dist * vec2(sin(t), cos(t)), 5.0);
        col = hsv(hue + r * 0.3 + n * 0.2, sat, smoothstep(0.2, 0.9, n) * (0.55 + flash * 1.3));
    } else if (u_scene < 3.5) {     // Espectro circular
        float r = length(p), a = atan(p.y, p.x);
        float x = abs(a) / 3.14159;
        float nb = 16.0 + floor(detail * 48.0);
        float e = bandAt(floor(x * nb) / nb);
        float rad = 0.35 + e * 0.55;
        float line = smoothstep(0.02, 0.0, abs(r - rad));
        float fill = smoothstep(rad, rad - 0.02, r) * step(0.33, r) * (0.35 + 0.65 * e);
        float gap = step(0.15, fract(x * nb));
        col = hsv(hue + x * 0.3, sat, (fill * gap * 0.8 + line) * (0.6 + flash * 1.2));
        col += hsv(hue + 0.5, sat * 0.7, 1.0) * flash * 0.5 * exp(-r * r * 8.0);
        col += hsv(hue, sat, 0.15) * smoothstep(0.33, 0.0, r) * (0.5 + bandAt(0.0));
    } else {                        // Ondas
        float v = 0.0;
        for (int i = 0; i < 8; i++) {
            float fi = float(i);
            float e = bandAt(fi / 7.0);
            float y = p.y + (fi - 3.5) * 0.18;
            float w = sin(p.x * (2.0 + detail * 8.0) + t * (1.0 + fi * 0.3) + fi) * (0.05 + 0.25 * dist) * (0.5 + e);
            v += smoothstep(0.025, 0.0, abs(y - w)) * (0.4 + 0.6 * e);
        }
        col = hsv(hue + p.y * 0.2, sat, v * (0.6 + flash * 1.2));
    }
    col = 1.0 - exp(-col * 1.8);    // evita blancos quemados
    FRAG = vec4(col, 1.0);
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

    fn paint(&self, gl: &glow::Context, u: &Uniforms, size: [f32; 2]) {
        unsafe {
            gl.use_program(Some(self.program));
            let loc = |name: &str| gl.get_uniform_location(self.program, name);
            gl.uniform_2_f32(loc("u_res").as_ref(), size[0], size[1]);
            gl.uniform_1_f32(loc("u_time").as_ref(), u.time);
            gl.uniform_1_f32(loc("u_rot").as_ref(), u.rot);
            gl.uniform_1_f32(loc("u_scene").as_ref(), u.scene);
            gl.uniform_1_f32_slice(loc("u_p").as_ref().or(loc("u_p[0]").as_ref()), &u.params);
            gl.uniform_1_f32_slice(loc("u_bands").as_ref().or(loc("u_bands[0]").as_ref()), &u.bands);
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
pub fn paint(ui: &egui::Ui, rect: egui::Rect, renderer: &Arc<Mutex<Renderer>>, u: Uniforms) {
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
