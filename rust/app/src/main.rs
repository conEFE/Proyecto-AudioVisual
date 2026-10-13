//! AV Visual: visualizador de música en vivo con editor, sin necesitar TouchDesigner.
//!
//! Atajos: H oculta/muestra el panel · F pantalla completa · 1-5 cambia de escena.
//! Arrastra un archivo de audio (WAV, MP3, FLAC) sobre la ventana para reproducirlo.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod demo;
mod engine;
mod mapping;
mod visuals;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use audiovisual::{Config, input};
use eframe::egui;

use engine::{Engine, InputKind, OscTarget};
use mapping::{Evaluator, PARAMS, Preset, SCENES, SOURCES};
use visuals::{Renderer, Uniforms};

const BAND_KEYS: [&str; 6] = [
    "band/sub",
    "band/bass",
    "band/lowmid",
    "band/mid",
    "band/highmid",
    "band/high",
];
const NOTE_NAMES: [&str; 12] = [
    "Do", "Do#", "Re", "Re#", "Mi", "Fa", "Fa#", "Sol", "Sol#", "La", "La#", "Si",
];

struct Args {
    demo: bool,
    scene: Option<usize>,
    hide_panel: bool,
}

fn parse_args() -> Args {
    let mut a = Args {
        demo: false,
        scene: None,
        hide_panel: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--demo" => a.demo = true,
            "--sin-panel" => a.hide_panel = true,
            "--escena" => {
                a.scene = it
                    .next()
                    .and_then(|s| s.parse::<usize>().ok())
                    .map(|n| n.saturating_sub(1))
            }
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
    eval: Evaluator,
    values: Vec<f32>,
    index: HashMap<String, usize>,
    time: f32,
    rot: f32,
    last_frame: Instant,
    show_panel: bool,
    fullscreen: bool,
    last_phase: f32,
    beats: u32,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>, args: Args) -> Self {
        let (renderer, render_error) = match cc.gl.as_ref().map(|gl| Renderer::new(gl)) {
            Some(Ok(r)) => (Some(Arc::new(Mutex::new(r))), None),
            Some(Err(e)) => (None, Some(e)),
            None => (None, Some("no hay OpenGL disponible".into())),
        };
        let cfg = Config::load(None).unwrap_or_else(|_| Config::builtin());
        let input = if args.demo {
            InputKind::Demo
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
            preset: Preset::default_for(args.scene.unwrap_or(0).min(SCENES.len() - 1)),
            saved: Vec::new(),
            save_name: String::new(),
            message: String::new(),
            renderer,
            render_error,
            eval: Evaluator::default(),
            values: Vec::new(),
            index: HashMap::new(),
            time: 0.0,
            rot: 0.0,
            last_frame: Instant::now(),
            show_panel: !args.hide_panel,
            fullscreen: false,
            last_phase: 0.0,
            beats: 0,
        };
        app.refresh_saved();
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
        match std::fs::read_to_string(&path)
            .map_err(anyhow::Error::from)
            .and_then(|t| Ok(serde_json::from_str::<Preset>(&t)?))
        {
            Ok(p) => {
                self.preset = p;
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

    /// Lee el audio y calcula los parámetros del cuadro actual.
    fn update_audio(&mut self) -> (Uniforms, String, Option<String>) {
        let mut status = String::from("sin audio");
        let mut error = None;
        if let Some(e) = &self.engine {
            let mut sh = e.shared.lock().unwrap();
            if self.index.len() != sh.names.len() {
                self.index = sh.names.iter().enumerate().map(|(i, n)| (n.clone(), i)).collect();
            }
            sh.take(&mut self.values);
            status = sh.status.clone();
            error = sh.error.clone();
        }
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        let vals = &self.values;
        let index = &self.index;
        let lookup = |k: &str| index.get(k).and_then(|&i| vals.get(i)).copied().unwrap_or(0.0);
        let p = self.eval.eval(&self.preset, dt, lookup);
        self.time += dt * p[1] * 2.0;
        self.rot += dt * p[7];

        // cambio de escena automático cada N compases
        let phase = self.value("tempo/phase");
        if phase < self.last_phase - 0.5 {
            self.beats += 1;
            let bars = self.preset.auto_scene_bars;
            if bars > 0 && self.beats >= bars * 4 {
                self.beats = 0;
                let next = (self.preset.scene + 1) % SCENES.len();
                let auto = bars;
                self.preset = Preset::default_for(next);
                self.preset.auto_scene_bars = auto;
            }
        }
        self.last_phase = phase;

        let mut bands = [0.0; 6];
        for (b, k) in bands.iter_mut().zip(BAND_KEYS) {
            *b = self.value(k);
        }
        let u = Uniforms {
            time: self.time,
            rot: self.rot,
            params: p,
            bands,
            scene: self.preset.scene as f32,
        };
        (u, status, error)
    }

    fn panel(&mut self, ui: &mut egui::Ui, status: &str, error: Option<&str>) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("AV Visual");
            ui.label(egui::RichText::new(status).weak());
            if let Some(e) = error {
                ui.colored_label(egui::Color32::from_rgb(255, 120, 100), e);
                if ui.button("Usar canción de demostración").clicked() {
                    self.start(InputKind::Demo);
                }
            }
            if let Some(e) = &self.render_error {
                ui.colored_label(egui::Color32::from_rgb(255, 120, 100), e);
            }
            ui.add_space(6.0);

            // --- Entrada de audio ---
            ui.collapsing("🎤 Entrada de audio", |ui| {
                let label = match &self.input {
                    InputKind::Live(None) => "Entrada por defecto".to_string(),
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
                };
                let mut choice: Option<InputKind> = None;
                egui::ComboBox::from_id_salt("entrada")
                    .width(250.0)
                    .selected_text(label)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(self.input == InputKind::Live(None), "Entrada por defecto")
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
                if ui.button("↻ Buscar dispositivos").clicked() {
                    self.devices = input::input_device_names();
                }
                ui.label(
                    egui::RichText::new("Para usar tus canciones, arrastra un archivo de audio a la ventana.")
                        .small()
                        .weak(),
                );
                if let Some(c) = choice {
                    self.start(c);
                }
            });

            // --- Escena ---
            ui.separator();
            ui.label(egui::RichText::new("Escena").strong());
            ui.horizontal_wrapped(|ui| {
                for (i, name) in SCENES.iter().enumerate() {
                    if ui
                        .selectable_label(self.preset.scene == i, format!("{} {name}", i + 1))
                        .clicked()
                        && self.preset.scene != i
                    {
                        let auto = self.preset.auto_scene_bars;
                        self.preset = Preset::default_for(i);
                        self.preset.auto_scene_bars = auto;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("Cambiar sola cada");
                ui.add(egui::DragValue::new(&mut self.preset.auto_scene_bars).range(0..=64));
                ui.label("compases (0 = nunca)");
            });

            // --- Mapeos ---
            ui.separator();
            ui.label(egui::RichText::new("Qué mueve cada cosa").strong());
            ui.label(
                egui::RichText::new("Elige qué parte del sonido controla cada parámetro de la imagen.")
                    .small()
                    .weak(),
            );
            for (i, (name, help)) in PARAMS.iter().enumerate() {
                let Some(m) = self.preset.params.get_mut(i) else {
                    continue;
                };
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(*name).strong()).on_hover_text(*help);
                    egui::ComboBox::from_id_salt(("fuente", i))
                        .width(170.0)
                        .selected_text(mapping::source_label(&m.source))
                        .show_ui(ui, |ui| {
                            for (key, label, _) in SOURCES {
                                ui.selectable_value(&mut m.source, key.to_string(), *label);
                            }
                        });
                });
                ui.add(egui::Slider::new(&mut m.base, 0.0..=1.0).text("base"));
                if !m.source.is_empty() {
                    ui.add(egui::Slider::new(&mut m.amount, 0.0..=2.0).text("cuánto"));
                    ui.add(egui::Slider::new(&mut m.smooth, 0.0..=2.0).text("calma (s)"));
                }
            }
            if ui.button("Restablecer esta escena").clicked() {
                self.preset = Preset::default_for(self.preset.scene);
            }

            // --- Presets ---
            ui.separator();
            ui.label(egui::RichText::new("Mis presets").strong());
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut self.save_name)
                        .hint_text("nombre")
                        .desired_width(150.0),
                );
                if ui.button("💾 Guardar").clicked() {
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
            ui.collapsing("📡 Enviar también a TouchDesigner (OSC)", |ui| {
                let before = (self.osc_on, self.osc_host.clone(), self.osc_port);
                ui.checkbox(&mut self.osc_on, "Enviar datos por OSC");
                ui.horizontal(|ui| {
                    ui.label("IP");
                    ui.add(egui::TextEdit::singleline(&mut self.osc_host).desired_width(110.0));
                    ui.label("puerto");
                    ui.add(egui::DragValue::new(&mut self.osc_port).range(1..=65535));
                });
                if before != (self.osc_on, self.osc_host.clone(), self.osc_port) && ui.button("Aplicar").clicked() {
                    self.start(self.input.clone());
                }
                if before.0 != self.osc_on {
                    self.start(self.input.clone());
                }
            });

            // --- Medidores ---
            ui.separator();
            ui.label(egui::RichText::new("Sonido ahora").strong());
            let note = if self.value("lead/confidence") > 0.3 {
                NOTE_NAMES[(self.value("lead/pitch_class") as usize).min(11)]
            } else {
                "-"
            };
            ui.label(format!("{:.0} BPM · nota {note}", self.value("tempo/bpm")));
            for (key, label) in [
                ("mix/level", "Volumen"),
                ("band/bass", "Graves"),
                ("band/mid", "Medios"),
                ("band/high", "Agudos"),
                ("drums/kick", "Bombo"),
                ("drums/snare", "Caja"),
                ("drums/hihat", "Hi-hat"),
                ("lead", "Melodía"),
            ] {
                ui.add(
                    egui::ProgressBar::new(self.value(key).clamp(0.0, 1.0))
                        .text(label)
                        .desired_height(12.0),
                );
            }
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("H: ocultar panel · F: pantalla completa · 1-5: escena")
                    .small()
                    .weak(),
            );
        });
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // atajos y archivos arrastrados
        let (toggle, full, scene_key, dropped) = ctx.input(|i| {
            let scene = [
                egui::Key::Num1,
                egui::Key::Num2,
                egui::Key::Num3,
                egui::Key::Num4,
                egui::Key::Num5,
            ]
            .iter()
            .position(|k| i.key_pressed(*k));
            (
                i.key_pressed(egui::Key::H),
                i.key_pressed(egui::Key::F) || i.key_pressed(egui::Key::F11),
                scene,
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
            if let Some(s) = scene_key
                && s != self.preset.scene
            {
                self.preset = Preset::default_for(s);
            }
        }
        if let Some(path) = dropped {
            self.start(InputKind::File(path));
        }

        let (uniforms, status, error) = self.update_audio();

        if self.show_panel {
            egui::Panel::left("panel")
                .resizable(true)
                .default_size(330.0)
                .show(ui, |ui| self.panel(ui, &status, error.as_deref()));
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            let rect = ui.max_rect();
            if let Some(r) = &self.renderer {
                visuals::paint(ui, rect, r, uniforms);
            }
            if !self.show_panel {
                // botón discreto para volver a mostrar el panel
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
