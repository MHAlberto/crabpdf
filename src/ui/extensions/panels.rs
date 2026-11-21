use super::timer::Timer;
use super::*;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io,
    path::Path,
    time::{Duration, Instant},
};

type Bindings = BTreeMap<String, [Option<String>; 2]>;

#[derive(Default)]
pub(super) struct Panels {
    pub(super) active: Option<String>,
    timers: BTreeMap<String, Timer>,
    bindings: Bindings,
    next_reload: Option<Instant>,
    recording: Option<(String, usize)>,
    shortcut_message: Option<String>,
}

const MODIFIERS: egui::Modifiers = egui::Modifiers::ALT.plus(egui::Modifiers::SHIFT);

fn read_bindings(path: &Path) -> io::Result<Bindings> {
    match crabpdf_extension_api::read_message(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Bindings::new()),
        result => result,
    }
}

fn save_binding(data: &Path, id: &str, slot: usize, key: Option<String>) -> io::Result<Bindings> {
    fs::create_dir_all(data)?;
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(data.join("extension-shortcuts.lock"))?;
    lock.try_lock().map_err(|_| {
        io::Error::other("Otra ventana está guardando atajos. Vuelve a intentarlo.")
    })?;
    let path = data.join("extension-shortcuts.json");
    let mut bindings = read_bindings(&path)?;
    if let Some(key) = &key {
        let installed = Repository::new(data).installed_ids()?;
        for (other_id, slots) in &mut bindings {
            for (other_slot, binding) in slots.iter_mut().enumerate() {
                if (other_id == id && other_slot == slot) || binding.as_ref() != Some(key) {
                    continue;
                }
                if other_id == id || installed.contains(other_id) {
                    return Err(io::Error::other("Ese atajo ya está en uso."));
                }
                // A newly assigned key supersedes the saved key of an uninstalled extension.
                *binding = None;
            }
        }
    }
    bindings.entry(id.into()).or_default()[slot] = key;
    bindings.retain(|_, slots| slots.iter().any(Option::is_some));
    crate::fileops::atomic_write(&path, |output| {
        serde_json::to_writer(output, &bindings).map_err(io::Error::other)
    })?;
    Ok(bindings)
}

impl Panels {
    fn load(&mut self, data: &Path, ctx: &egui::Context) {
        let now = Instant::now();
        let next = self
            .next_reload
            .filter(|next| *next > now)
            .unwrap_or_else(|| {
                if let Ok(bindings) = read_bindings(&data.join("extension-shortcuts.json")) {
                    self.bindings = bindings;
                }
                now + Duration::from_secs(1)
            });
        self.next_reload = Some(next);
        if self.active.is_some() {
            ctx.request_repaint_after(next.saturating_duration_since(now));
        }
    }

    fn set_binding(&mut self, data: &Path, id: &str, slot: usize, key: Option<String>) {
        match save_binding(data, id, slot, key) {
            Ok(bindings) => {
                self.bindings = bindings;
                self.shortcut_message = None;
            }
            Err(error) => self.shortcut_message = Some(error.to_string()),
        }
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
        self.load(data, ctx);
        self.timers.retain(|id, timer| {
            let settings = packages
                .iter()
                .find(|p| p.enabled && &p.manifest.id == id)
                .and_then(|package| package.manifest.timer);
            if let Some(settings) = settings {
                if timer.settings != settings {
                    *timer = Timer::new(settings);
                }
                true
            } else {
                false
            }
        });
        for package in packages.iter().filter(|package| package.enabled) {
            if let Some(settings) = package.manifest.timer {
                self.timers
                    .entry(package.manifest.id.clone())
                    .or_insert_with(|| Timer::new(settings));
            }
        }
        if self
            .active
            .as_ref()
            .is_some_and(|id| !packages.iter().any(|p| p.enabled && &p.manifest.id == id))
        {
            self.close();
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
                    self.set_binding(data, &id, slot, Some(key.name().into()));
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
                    if !Repository::new(data).is_active(package) {
                        return;
                    }
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

    pub(super) fn shortcuts(&mut self, ui: &mut egui::Ui, id: &str, data: &Path) {
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
                        self.set_binding(data, id, slot, None);
                    }
                });
            }
            if self
                .recording
                .as_ref()
                .is_some_and(|(owner, _)| owner == id)
            {
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

    pub(super) fn clock(&self, id: &str) -> Option<(&Timer, String)> {
        let timer = self.timers.get(id)?;
        Some((timer, timer.clock()))
    }

    pub(super) fn tick_timers(&mut self, ctx: &egui::Context) -> (u64, Option<&'static str>) {
        let mut count = 0;
        let mut notice = None;
        for timer in self.timers.values_mut() {
            if let Some(completion) = timer.tick(ctx) {
                count += u64::from(completion.focus);
                notice = Some(completion.message);
            }
        }
        (count, notice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_from_two_windows_merge_and_uninstalled_keys_can_be_reused() {
        let data = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("artifacts/extension-tests")
            .join(format!("shortcuts-{:016x}", getrandom::u64().unwrap()));
        let first = "org.crabpdf.first";
        let second = "org.crabpdf.second";
        for id in [first, second] {
            fs::create_dir_all(data.join("extensions/installed").join(id).join("0.1.0")).unwrap();
        }
        let mut window_a = Panels::default();
        let mut window_b = Panels::default();
        let ctx = egui::Context::default();
        window_a.load(&data, &ctx);
        window_b.load(&data, &ctx);
        window_a.set_binding(&data, first, 0, Some("A".into()));
        window_b.set_binding(&data, second, 0, Some("B".into()));
        let path = data.join("extension-shortcuts.json");
        let written = read_bindings(&path).unwrap();
        assert_eq!(written[first][0].as_deref(), Some("A"));
        assert_eq!(written[second][0].as_deref(), Some("B"));
        assert!(save_binding(&data, second, 1, Some("A".into())).is_err());

        fs::remove_dir(data.join("extensions/installed").join(first).join("0.1.0")).unwrap();
        window_b.set_binding(&data, second, 1, Some("A".into()));
        window_a.next_reload = None;
        window_a.load(&data, &ctx);
        assert!(!window_a.bindings.contains_key(first));
        assert_eq!(window_a.bindings[second][1].as_deref(), Some("A"));
    }
}
