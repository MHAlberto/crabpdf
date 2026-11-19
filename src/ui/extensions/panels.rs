use super::*;
use crabpdf_extension_api::TimerSpec;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

type Bindings = BTreeMap<String, [Option<String>; 2]>;

#[derive(Default)]
pub(super) struct Panels {
    pub(super) active: Option<String>,
    timers: BTreeMap<String, Timer>,
    bindings: Bindings,
    bindings_path: Option<PathBuf>,
    recording: Option<(String, usize)>,
    shortcut_message: Option<String>,
}

struct Timer {
    settings: TimerSpec,
    remaining: Duration,
    deadline: Option<Instant>,
    resting: bool,
    notice: Option<&'static str>,
}

impl Timer {
    fn new(settings: TimerSpec) -> Self {
        Self {
            settings,
            remaining: Duration::from_secs(u64::from(settings.focus_minutes) * 60),
            deadline: None,
            resting: false,
            notice: None,
        }
    }

    fn reset(&mut self) {
        *self = Self::new(self.settings);
    }

    fn tick(&mut self, ctx: &egui::Context) {
        let Some(deadline) = self.deadline else {
            return;
        };
        self.remaining = deadline.saturating_duration_since(Instant::now());
        if self.remaining.is_zero() {
            self.deadline = None;
            self.resting = !self.resting;
            let (minutes, notice) = if self.resting {
                (
                    self.settings.break_minutes,
                    "Terminó el enfoque. Es hora de descansar.",
                )
            } else {
                (
                    self.settings.focus_minutes,
                    "Terminó el descanso. Puedes volver a enfocarte.",
                )
            };
            self.remaining = Duration::from_secs(u64::from(minutes) * 60);
            self.notice = Some(notice);
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                egui::UserAttentionType::Informational,
            ));
        } else {
            ctx.request_repaint_after(self.remaining.min(Duration::from_secs(1)));
        }
    }

    fn show(&mut self, ui: &mut egui::Ui) {
        let secs = self.remaining.as_secs() + u64::from(self.remaining.subsec_nanos() != 0);
        ui.label(if self.resting { "Descanso" } else { "Enfoque" });
        ui.label(
            egui::RichText::new(format!("{:02}:{:02}", secs / 60, secs % 60))
                .monospace()
                .size(32.0),
        );
        ui.horizontal(|ui| {
            if let Some(deadline) = self.deadline {
                if ui.button("Pausar").clicked() {
                    self.remaining = deadline.saturating_duration_since(Instant::now());
                    self.deadline = None;
                }
            } else if ui.button("Iniciar").clicked() {
                self.notice = None;
                self.deadline = Some(Instant::now() + self.remaining);
                ui.ctx().request_repaint();
            }
            if ui.button("Reiniciar").clicked() {
                self.reset();
            }
        });
    }
}

const MODIFIERS: egui::Modifiers = egui::Modifiers::ALT.plus(egui::Modifiers::SHIFT);

impl Panels {
    fn load(&mut self, data: &std::path::Path) {
        if self.bindings_path.is_some() {
            return;
        }
        let path = data.join("extension-shortcuts.json");
        self.bindings = crabpdf_extension_api::read_message(&path).unwrap_or_default();
        self.bindings_path = Some(path);
    }

    fn set_binding(&mut self, id: &str, slot: usize, key: Option<String>) {
        if let Some(k) = key.as_ref()
            && self.bindings.iter().any(|(other_id, slots)| {
                slots.iter().enumerate().any(|(other_slot, other)| {
                    (other_id != id || other_slot != slot) && other.as_ref() == Some(k)
                })
            })
        {
            self.shortcut_message = Some("Ese atajo ya está en uso.".into());
            return;
        }
        let mut bindings = self.bindings.clone();
        bindings.entry(id.into()).or_default()[slot] = key;
        if let Some(path) = &self.bindings_path {
            let saved = crate::fileops::atomic_write(path, |output| {
                serde_json::to_writer(output, &bindings).map_err(std::io::Error::other)
            });
            if let Err(error) = saved {
                self.shortcut_message = Some(format!("No se guardó el atajo. {error}"));
                return;
            }
        }
        self.bindings = bindings;
        self.shortcut_message = None;
    }

    pub(super) fn open(&mut self, package: &InstalledPackage) {
        if let Some(settings) = package.manifest.timer {
            self.timers
                .entry(package.manifest.id.clone())
                .or_insert_with(|| Timer::new(settings));
        }
        self.active = Some(package.manifest.id.clone());
        self.recording = None;
        self.shortcut_message = None;
    }

    pub(in crate::ui) fn close(&mut self) -> bool {
        self.recording = None;
        self.active.take().is_some()
    }

    pub(super) fn poll(
        &mut self,
        ctx: &egui::Context,
        data: &std::path::Path,
        packages: &[InstalledPackage],
    ) {
        self.load(data);
        self.timers
            .retain(|id, _| packages.iter().any(|p| p.enabled && &p.manifest.id == id));
        if self
            .active
            .as_ref()
            .is_some_and(|id| !packages.iter().any(|p| p.enabled && &p.manifest.id == id))
        {
            self.close();
        }
        for timer in self.timers.values_mut() {
            timer.tick(ctx);
        }

        if let Some((id, slot)) = self.recording.clone() {
            let key = ctx.input_mut(|input| {
                let index = input.events.iter().position(|event| {
                    matches!(event,
                        egui::Event::Key { key, modifiers, pressed: true, repeat: false, .. }
                            if *key == egui::Key::Escape || modifiers.matches_exact(MODIFIERS)
                    )
                })?;
                let egui::Event::Key { key, .. } = input.events.remove(index) else {
                    return None;
                };
                Some(key)
            });
            if let Some(key) = key {
                self.recording = None;
                if key != egui::Key::Escape {
                    self.set_binding(&id, slot, Some(key.name().into()));
                }
            }
            return;
        }

        if ctx.text_edit_focused() || egui::Popup::is_any_open(ctx) {
            return;
        }
        for package in packages.iter().filter(|p| p.enabled) {
            let Some(slots) = self.bindings.get(&package.manifest.id) else {
                continue;
            };
            for (slot, binding) in slots.iter().enumerate() {
                let Some(key) = binding.as_deref().and_then(egui::Key::from_name) else {
                    continue;
                };
                if ctx.input_mut(|input| input.consume_key(MODIFIERS, key)) {
                    if slot == 0 {
                        self.open(package);
                    } else if self.active.as_deref() == Some(package.manifest.id.as_str()) {
                        self.close();
                    }
                    return;
                }
            }
        }
    }

    pub(super) fn shortcuts(&mut self, ui: &mut egui::Ui, id: &str) {
        ui.collapsing("Atajos de teclado", |ui| {
            for (slot, label) in ["Abrir", "Cerrar"].into_iter().enumerate() {
                ui.horizontal(|ui| {
                    let bound = self
                        .bindings
                        .get(id)
                        .and_then(|slots| slots[slot].as_deref());
                    let name = bound
                        .map_or_else(|| "Asignar atajo".into(), |key| format!("Alt+Mayús+{key}"));
                    ui.label(label);
                    if ui.button(name).clicked() {
                        self.recording = Some((id.into(), slot));
                        self.shortcut_message = None;
                    }
                    if bound.is_some() && ui.small_button("Quitar").clicked() {
                        self.set_binding(id, slot, None);
                    }
                });
            }
            if self.recording.is_some() {
                ui.label("Pulsa Alt+Mayús y la tecla que quieras. Esc cancela.");
            }
            if let Some(message) = &self.shortcut_message {
                ui.label(message);
            }
        });
    }

    pub(super) fn timer(&mut self, ui: &mut egui::Ui, id: &str) {
        if let Some(timer) = self.timers.get_mut(id) {
            timer.show(ui);
        }
    }

    pub(super) fn notifications(&mut self, ctx: &egui::Context) {
        for (id, timer) in &mut self.timers {
            let Some(message) = timer.notice else {
                continue;
            };
            egui::Window::new("Pomodoro")
                .id(egui::Id::new(("timer-notice", id)))
                .anchor(egui::Align2::RIGHT_BOTTOM, [-18.0, -18.0])
                .resizable(false)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(message);
                    if ui.button("Entendido").clicked() {
                        timer.notice = None;
                    }
                });
        }
    }
}
