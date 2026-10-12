//! Envío de los datos de análisis por OSC (TouchDesigner, Resolume, Max, etc.).

use std::net::{ToSocketAddrs, UdpSocket};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use rosc::{OscMessage, OscPacket, OscType, encoder};

use crate::analyzer::Features;

struct Group {
    addrs: Vec<String>,
    pending: Vec<f32>,
    pulse: Vec<bool>,
    fresh: bool,
}

pub struct OscSender {
    socket: UdpSocket,
    target: std::net::SocketAddr,
    prefix: String,
    interval: Duration,
    last: Option<Instant>,
    groups: Vec<Group>,
    buf: Vec<u8>,
}

impl OscSender {
    pub fn new(host: &str, port: u16, prefix: &str, send_fps: f32) -> Result<Self> {
        let target = (host, port)
            .to_socket_addrs()?
            .next()
            .with_context(|| format!("dirección inválida {host}:{port}"))?;
        let bind = if target.is_ipv4() { "0.0.0.0:0" } else { "[::]:0" };
        let p = prefix.trim_matches('/');
        Ok(Self {
            socket: UdpSocket::bind(bind)?,
            target,
            prefix: if p.is_empty() { String::new() } else { format!("/{p}") },
            interval: if send_fps > 0.0 {
                Duration::from_secs_f32(1.0 / send_fps)
            } else {
                Duration::ZERO
            },
            last: None,
            groups: Vec::new(),
            buf: Vec::with_capacity(256),
        })
    }

    /// Registra un juego de valores (ej. la mezcla, o un canal `ch/voz`) y devuelve su id.
    pub fn register(&mut self, scope: &str, features: &Features) -> usize {
        let scope = scope.trim_matches('/');
        let base = if scope.is_empty() {
            self.prefix.clone()
        } else {
            format!("{}/{scope}", self.prefix)
        };
        self.groups.push(Group {
            addrs: features.names.iter().map(|n| format!("{base}/{n}")).collect(),
            pending: vec![0.0; features.names.len()],
            pulse: features.pulse.clone(),
            fresh: true,
        });
        self.groups.len() - 1
    }

    /// Acumula valores hasta el próximo envío.
    ///
    /// Los pulsos (golpes) se guardan como el máximo desde el último envío,
    /// así un golpe corto nunca se pierde aunque se envíe a menos fps.
    pub fn send(&mut self, group: usize, features: &Features) {
        let g = &mut self.groups[group];
        for (i, &v) in features.values.iter().enumerate() {
            g.pending[i] = if g.pulse[i] && !g.fresh { g.pending[i].max(v) } else { v };
        }
        g.fresh = false;
    }

    /// Envía lo acumulado si ya pasó el intervalo (o siempre con `force`).
    pub fn flush(&mut self, force: bool) -> Result<bool> {
        let now = Instant::now();
        if !force && self.last.is_some_and(|t| now - t < self.interval) {
            return Ok(false);
        }
        for g in &mut self.groups {
            if g.fresh {
                continue; // nada nuevo desde el último envío
            }
            for (addr, &v) in g.addrs.iter().zip(&g.pending) {
                let packet = OscPacket::Message(OscMessage {
                    addr: addr.clone(),
                    args: vec![OscType::Float(v)],
                });
                self.buf.clear();
                encoder::encode_into(&packet, &mut self.buf)?;
                // UDP: si nadie escucha no es un error para nosotros
                let _ = self.socket.send_to(&self.buf, self.target);
            }
            g.fresh = true;
        }
        self.last = Some(now);
        Ok(true)
    }
}
