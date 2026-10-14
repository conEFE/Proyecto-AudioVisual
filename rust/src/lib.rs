//! Visualizador de audio en vivo: análisis en tiempo real + envío OSC (TouchDesigner, etc.).

pub mod analyzer;
pub mod config;
pub mod drums;
pub mod dsp;
pub mod input;
pub mod osc;

pub use analyzer::{Features, LiveAnalyzer};
pub use config::Config;
pub use osc::OscSender;
