//! AV Visual: cada instrumento de la música tiene su propia capa visual, con estética glitch.
//!
//! Atajos: H oculta/muestra el panel · F pantalla completa · 1-6 enciende/apaga cada capa ·
//! G más/menos glitch. Arrastra un archivo de audio (WAV, MP3, FLAC) a la ventana para reproducirlo.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod band;
mod demo;
mod engine;
mod visuals;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use audiovisual::{Config, input};
use eframe::egui;

use band::{Activity, BandState, DRUM_LAYERS, LAYERS, PALETTES, Preset};
use engine::{Engine, InputKind, OscTarget};
use visuals::Renderer;

const NOTE_NAMES: [&str; 12] = [
    "Do", "Do#", "Re", "Re#", "Mi", "Fa", "Fa#", "Sol", "Sol#", "La", "La#", "Si",
];

struct Args {
    demo: bool,
    hide_panel: bool,
    preset: Option<String>,
}

fn parse_args() -> Args {
    let mut a = Args {
        demo: false,
        hide_panel: false,
        preset: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--demo" => a.demo = true,
            "--sin-panel" => a.hide_panel = true,
            "--preset" => a.preset = it.next(),
            _ => {}
        }
    }
    a
}

fn presets_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("presets")))
        .unwrap_or_else(|| PathBuf::from("presets"))
}

fn color32(c: [f32; 3], a: f32) -> egui::Color32 {
    let f = |v: f32| (v.clamp(0.0, 1.0) * 255.0) as u8;
    egui::Color32::from_rgba_unmultiplied(f(c[0]), f(c[1]), f(c[2]), f(a))
}

struct App {
    cfg: Config,
    engine: Option<Engine>,
    input: InputKind,
    devices: Vec<String>,
    osc_on: bool,
    osc_host: String,
    osc_port: u16,

    preset: Preset,
    saved: Vec<String>,
    save_name: String,
    message: String,

    renderer: Option<Arc<Mutex<Renderer>>>,
    render_error: Option<String>,
    state: BandState,
    activity: Activity,
    values: Vec<f32>,
    index: HashMap<String, usize>,
    last_frame: Instant,
    show_panel: bool,
    fullscreen: bool,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
        let (renderer, render_error) = match cc.gl.as_ref().map(|gl| Renderer::new(gl)) {
            Some(Ok(r)) => (Some(Arc::new(Mutex::new(r))), None),
            Some(Err(e)) => (None, Some(e)),
            None => (None, Some("no hay OpenGL disponible".into())),
        };
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = egui::Color32::from_rgb(8, 8, 10);
        visuals.window_fill = egui::Color32::from_rgb(8, 8, 10);
        cc.egui_ctx.set_visuals(visuals);

        let cfg = Config::load(None).unwrap_or_else(|_| Config::builtin());
        let input = if args.demo {
            InputKind::Demo
        } else if cfg!(windows) {
            InputKind::System
        } else {
            InputKind::Live(None)
        };
        let mut app = Self {
            osc_host: cfg.osc.host.clone(),
            osc_port: cfg.osc.port,
            cfg,
            engine: None,
            input: input.clone(),
            devices: input::input_device_names(),
            osc_on: false,
            preset: Preset::default(),
            saved: Vec::new(),
            save_name: String::new(),
            message: String::new(),
            renderer,
            render_error,
            state: BandState::default(),
            activity: [0.0; 6],
            values: Vec::new(),
            index: HashMap::new(),
            last_frame: Instant::now(),
            show_panel: !args.hide_panel,
            fullscreen: false,
        };
        app.refresh_saved();
        if let Some(name) = &args.preset {
            app.load_preset(name);
        }
        app.start(input);
        app
    }

    fn start(&mut self, kind: InputKind) {
        if let Some(mut e) = self.engine.take() {
            e.stop();
        }
        let osc = self.osc_on.then(|| OscTarget {
            host: self.osc_host.clone(),
            port: self.osc_port,
        });
        self.input = kind.clone();
        self.index.clear();
        self.engine = Some(Engine::start(kind, osc, self.cfg.clone()));
    }

    fn refresh_saved(&mut self) {
        self.saved = std::fs::read_dir(presets_dir())
            .map(|rd| {
                let mut v: Vec<String> = rd
                    .filter_map(|e| e.ok())
                    .filter_map(|e| {
                        let p = e.path();
                        if p.extension()? != "json" {
                            return None;
                        }
                        Some(p.file_stem()?.to_string_lossy().into_owned())
                    })
                    .collect();
                v.sort();
                v
            })
            .unwrap_or_default();
    }

    fn save_preset(&mut self) {
        let name = self.save_name.trim().to_string();
        if name.is_empty() {
            self.message = "Escribe un nombre para guardar.".into();
            return;
        }
        let dir = presets_dir();
        let mut p = self.preset.clone();
        p.name = name.clone();
        let res = std::fs::create_dir_all(&dir)
            .map_err(anyhow::Error::from)
            .and_then(|_| Ok(serde_json::to_string_pretty(&p)?))
            .and_then(|json| Ok(std::fs::write(dir.join(format!("{name}.json")), json)?));
        self.message = match res {
            Ok(()) => format!("Guardado \"{name}\"."),
            Err(e) => format!("No se pudo guardar: {e}"),
        };
        self.preset.name = name;
        self.refresh_saved();
    }

    fn load_preset(&mut self, name: &str) {
        let path = presets_dir().join(format!("{name}.json"));
        let loaded = std::fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|t| Ok(serde_json::from_str::<Preset>(&t)?));
        match loaded {
            Ok(p) => {
                self.preset = p.normalized();
                self.message = format!("Cargado \"{name}\".");
            }
            Err(e) => self.message = format!("No se pudo cargar: {e}"),
        }
    }

    fn value(&self, key: &str) -> f32 {
        self.index
            .get(key)
            .and_then(|&i| self.values.get(i))
            .copied()
            .unwrap_or(0.0)
    }

    /// Lee el audio y calcula lo que dibuja el cuadro actual.
    fn update_audio(&mut self) -> (band::BandUniforms, String, Option<String>) {
        let mut status = String::from("sin audio");
        let mut error = None;
        if let Some(e) = &self.engine {
            let mut sh = e.shared.lock().unwrap();
            if self.index.len() != sh.names.len() {
                self.index = sh.names.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect();
            }
            sh.take(&mut self.values);
            for (i, s) in sh.sensitivity.iter_mut().enumerate() {
                *s = self.preset.layers[i].sensitivity;
            }
            status = sh.status.clone();
            error = sh.error.clone();
        }
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;
        let (vals, index) = (&self.values, &self.index);
        let get = |k: &str| index.get(k).and_then(|&i| vals.get(i)).copied().unwrap_or(0.0);
        let (u, activity) = self.state.update(&self.preset, dt, get);
        self.activity = activity;
        (u, status, error)
    }

    fn panel(&mut self, ui: &mut egui::Ui, status: &str, error: Option<&str>) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(
                egui::RichText::new("AV VISUAL // BANDA")
                    .monospace()
                    .strong()
                    .size(16.0),
            );
            ui.label(egui::RichText::new(status).monospace().weak().small());
            if let Some(e) = error {
                ui.colored_label(egui::Color32::from_rgb(255, 80, 80), e);
                if ui.button("Usar canción de demostración").clicked() {
                    self.start(InputKind::Demo);
                }
            }
            if self.input == InputKind::System && error.is_none() && self.value("mix/level") < 0.01 {
                ui.label(
                    egui::RichText::new(
                        "Pon música en Spotify, YouTube o cualquier programa. Se captura lo que sale \
                         por los parlantes o audífonos que Windows usa por defecto.",
                    )
                    .small(),
                );
            }
            if let Some(e) = &self.render_error {
                ui.colored_label(egui::Color32::from_rgb(255, 80, 80), e);
            }
            ui.add_space(4.0);
            self.input_section(ui);

            // --- Capas: una por instrumento ---
            ui.separator();
            ui.label(egui::RichText::new("INSTRUMENTOS").monospace().strong());
            ui.label(
                egui::RichText::new("Cada uno tiene su capa. La barra muestra cuándo suena.")
                    .small()
                    .weak(),
            );
            for i in 0..LAYERS.len() {
                self.layer_ui(ui, i);
            }

            // --- Paleta ---
            ui.separator();
            ui.label(egui::RichText::new("PALETA").monospace().strong());
            ui.horizontal_wrapped(|ui| {
                for (i, (name, _)) in PALETTES.iter().enumerate() {
                    if ui.button(*name).clicked() {
                        self.preset.apply_palette(i);
                    }
                }
            });

            // --- Glitch ---
            ui.separator();
            ui.label(egui::RichText::new("GLITCH").monospace().strong());
            let g = &mut self.preset.glitch;
            ui.add(egui::Slider::new(&mut g.base, 0.0..=1.0).text("siempre"))
                .on_hover_text("Glitch permanente, aunque no haya golpes");
            ui.add(egui::Slider::new(&mut g.hits, 0.0..=1.0).text("con golpes"))
                .on_hover_text("Cuánto glitch suman la caja y el bombo");
            ui.add(egui::Slider::new(&mut g.chaos, 0.0..=1.0).text("caos"))
                .on_hover_text("Pixelado, bloques desplazados e inversiones");
            ui.add(egui::Slider::new(&mut g.scanlines, 0.0..=1.0).text("líneas de TV"));
            ui.horizontal(|ui| {
                ui.checkbox(&mut g.grid, "rejilla");
                ui.checkbox(&mut g.legend, "leyenda");
            });
            if ui.button("Restablecer todo").clicked() {
                self.preset = Preset::default();
            }

            // --- Presets ---
            ui.separator();
            ui.label(egui::RichText::new("MIS PRESETS").monospace().strong());
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.save_name)
                        .hint_text("nombre")
                        .desired_width(150.0),
                );
                if ui.button("Guardar").clicked() {
                    self.save_preset();
                }
            });
            let mut to_load = None;
            ui.horizontal_wrapped(|ui| {
                for name in &self.saved {
                    if ui.button(name).clicked() {
                        to_load = Some(name.clone());
                    }
                }
            });
            if let Some(n) = to_load {
                self.load_preset(&n);
            }
            if !self.message.is_empty() {
                ui.label(egui::RichText::new(&self.message).small());
            }

            // --- OSC ---
            ui.separator();
            ui.collapsing("Enviar también a TouchDesigner (OSC)", |ui| {
                let before = self.osc_on;
                ui.checkbox(&mut self.osc_on, "Enviar datos por OSC");
                ui.horizontal(|ui| {
                    ui.label("IP");
                    ui.add(egui::TextEdit::singleline(&mut self.osc_host).desired_width(110.0));
                    ui.label("puerto");
                    ui.add(egui::DragValue::new(&mut self.osc_port).range(1..=65535));
                });
                if (before != self.osc_on) || (self.osc_on && ui.button("Aplicar IP/puerto").clicked()) {
                    self.start(self.input.clone());
                }
            });

            ui.separator();
            let note = if self.value("lead/confidence") > 0.3 {
                NOTE_NAMES[(self.value("lead/pitch_class") as usize).min(11)]
            } else {
                "-"
            };
            ui.label(egui::RichText::new(format!("{:.0} BPM · nota {note}", self.value("tempo/bpm"))).monospace());
            ui.label(
                egui::RichText::new("H panel · F pantalla completa · 1-6 capas · G glitch")
                    .small()
                    .weak(),
            );
        });
    }

    fn input_section(&mut self, ui: &mut egui::Ui) {
        ui.collapsing("Entrada de audio", |ui| {
            let label = match &self.input {
                InputKind::Live(None) => "Micrófono / entrada por defecto".to_string(),
                InputKind::Live(Some(i)) => self
                    .devices
                    .get(*i)
                    .cloned()
                    .unwrap_or_else(|| format!("Dispositivo {i}")),
                InputKind::File(p) => p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                InputKind::Demo => "Canción de demostración".to_string(),
                InputKind::System => "Lo que suena en la PC".to_string(),
            };
            let mut choice: Option<InputKind> = None;
            egui::ComboBox::from_id_salt("entrada")
                .width(250.0)
                .selected_text(label)
                .show_ui(ui, |ui| {
                    if cfg!(windows)
                        && ui
                            .selectable_label(
                                self.input == InputKind::System,
                                "Lo que suena en la PC (Spotify, YouTube…)",
                            )
                            .clicked()
                    {
                        choice = Some(InputKind::System);
                    }
                    if ui
                        .selectable_label(self.input == InputKind::Live(None), "Micrófono / entrada por defecto")
                        .clicked()
                    {
                        choice = Some(InputKind::Live(None));
                    }
                    for (i, name) in self.devices.iter().enumerate() {
                        if ui
                            .selectable_label(self.input == InputKind::Live(Some(i)), name)
                            .clicked()
                        {
                            choice = Some(InputKind::Live(Some(i)));
                        }
                    }
                    if ui
                        .selectable_label(self.input == InputKind::Demo, "Canción de demostración")
                        .clicked()
                    {
                        choice = Some(InputKind::Demo);
                    }
                });
            if ui.button("Buscar dispositivos").clicked() {
                self.devices = input::input_device_names();
            }
            ui.label(
                egui::RichText::new("Para usar un archivo, arrástralo (WAV, MP3, FLAC) a la ventana.")
                    .small()
                    .weak(),
            );
            if let Some(c) = choice {
                self.start(c);
            }
        });
    }

    fn layer_ui(&mut self, ui: &mut egui::Ui, i: usize) {
        let (name, help, styles) = LAYERS[i];
        let act = self.activity[i];
        let layer = &mut self.preset.layers[i];
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.checkbox(&mut layer.on, "");
            ui.color_edit_button_rgb(&mut layer.color);
            ui.label(egui::RichText::new(format!("{} {name}", i + 1)).monospace().strong())
                .on_hover_text(help);
            let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 8.0), egui::Sense::hover());
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_gray(30));
            let mut fill = rect;
            fill.set_width(rect.width() * act.clamp(0.0, 1.0));
            ui.painter().rect_filled(fill, 0.0, color32(layer.color, 1.0));
        });
        if layer.on {
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                for (s, label) in styles.iter().enumerate() {
                    ui.selectable_value(&mut layer.style, s, *label);
                }
            });
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.add(
                    egui::Slider::new(&mut layer.intensity, 0.0..=2.0)
                        .text("brillo")
                        .show_value(false),
                );
                ui.add(
                    egui::Slider::new(&mut layer.size, 0.0..=2.0)
                        .text("tamaño")
                        .show_value(false),
                );
            });
            let help = if i < DRUM_LAYERS {
                "Más a la derecha: detecta golpes más suaves. Más a la izquierda: sólo los fuertes."
            } else {
                "Más a la derecha: reacciona aunque suene bajito."
            };
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.add(
                    egui::Slider::new(&mut layer.sensitivity, 0.0..=1.0)
                        .text("sensibilidad")
                        .show_value(false),
                )
                .on_hover_text(help);
            });
        }
    }

    /// Leyenda abajo a la izquierda: qué color es cada instrumento y cuándo suena.
    fn legend(&self, ui: &egui::Ui, rect: egui::Rect) {
        let painter = ui.painter_at(rect);
        // arriba a la derecha: pulso, tempo y nota
        let note = if self.value("lead/pitch_hz") > 0.0 && self.activity[4] > 0.05 {
            NOTE_NAMES[(self.value("lead/pitch_class") as usize).min(11)]
        } else {
            "--"
        };
        let beat = self.value("tempo/beat") > 0.5;
        painter.text(
            egui::pos2(rect.right() - 14.0, rect.top() + 14.0),
            egui::Align2::RIGHT_TOP,
            format!(
                "{} {:>3.0} BPM  {:<4}",
                if beat { "■" } else { "□" },
                self.value("tempo/bpm"),
                note
            ),
            egui::FontId::monospace(12.0),
            egui::Color32::from_white_alpha(170),
        );
        let font = egui::FontId::monospace(11.0);
        let mut x = rect.left() + 14.0;
        let y = rect.bottom() - 18.0;
        for (i, (name, _, _)) in LAYERS.iter().enumerate() {
            let l = &self.preset.layers[i];
            if !l.on {
                continue;
            }
            let a = 0.25 + 0.75 * self.activity[i].clamp(0.0, 1.0);
            let sq = egui::Rect::from_center_size(egui::pos2(x + 4.0, y), egui::vec2(8.0, 8.0));
            painter.rect_filled(sq, 0.0, color32(l.color, a));
            let text = painter.text(
                egui::pos2(x + 12.0, y),
                egui::Align2::LEFT_CENTER,
                name.to_uppercase(),
                font.clone(),
                egui::Color32::from_white_alpha((90.0 + 165.0 * a) as u8),
            );
            x = text.right() + 16.0;
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let keys = [
            egui::Key::Num1,
            egui::Key::Num2,
            egui::Key::Num3,
            egui::Key::Num4,
            egui::Key::Num5,
            egui::Key::Num6,
        ];
        let (toggle, full, glitch, layer_key, dropped) = ctx.input(|i| {
            (
                i.key_pressed(egui::Key::H),
                i.key_pressed(egui::Key::F) || i.key_pressed(egui::Key::F11),
                i.key_pressed(egui::Key::G),
                keys.iter().position(|k| i.key_pressed(*k)),
                i.raw
                    .dropped_files
                    .iter()
                    .map(|f| f.path().to_path_buf())
                    .find(|p| !p.as_os_str().is_empty()),
            )
        });
        let editing_text = ctx.memory(|m| m.focused().is_some());
        if !editing_text {
            if toggle {
                self.show_panel = !self.show_panel;
            }
            if full {
                self.fullscreen = !self.fullscreen;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
            }
            if glitch {
                let g = &mut self.preset.glitch;
                g.base = if g.base > 0.3 { 0.05 } else { 0.6 };
            }
            if let Some(k) = layer_key {
                self.preset.layers[k].on = !self.preset.layers[k].on;
            }
        }
        if let Some(path) = dropped {
            self.start(InputKind::File(path));
        }

        let (uniforms, status, error) = self.update_audio();

        if self.show_panel {
            egui::Panel::left("panel")
                .resizable(true)
                .default_size(320.0)
                .show(ui, |ui| self.panel(ui, &status, error.as_deref()));
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            let rect = ui.max_rect();
            if let Some(r) = &self.renderer {
                visuals::paint(ui, rect, r, uniforms);
            }
            if self.preset.glitch.legend {
                self.legend(ui, rect);
            }
            if !self.show_panel {
                let btn = egui::Rect::from_min_size(rect.min + egui::vec2(8.0, 8.0), egui::vec2(28.0, 24.0));
                if ui.put(btn, egui::Button::new("☰")).clicked() {
                    self.show_panel = true;
                }
            }
        });
        ctx.request_repaint();
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let Some(mut e) = self.engine.take() {
            e.stop();
        }
        if let (Some(gl), Some(r)) = (gl, &self.renderer) {
            r.lock().unwrap().destroy(gl);
        }
    }
}

fn main() -> eframe::Result {
    let args = parse_args();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("AV Visual")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([640.0, 400.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "AV Visual",
        options,
        Box::new(move |cc| Ok(Box::new(App::new(cc, args)))),
    )
}
