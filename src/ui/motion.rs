//! Short, interruptible UI motion. It never blends two different document pages.
use eframe::egui::{self, Vec2};

pub(super) const DURATION: f32 = 0.16;

#[derive(Clone, Copy)]
pub(super) struct ScrollMotion {
    from: Vec2,
    to: Vec2,
    started: f64,
}
impl ScrollMotion {
    pub fn new(from: Vec2, to: Vec2, now: f64) -> Self {
        Self {
            from,
            to,
            started: now,
        }
    }
    pub fn sample(self, now: f64) -> (Vec2, bool) {
        let t = ((now - self.started) / f64::from(DURATION)).clamp(0.0, 1.0) as f32;
        let eased = t * t * (3.0 - 2.0 * t);
        (self.from + (self.to - self.from) * eased, t < 1.0)
    }
    pub fn shift(&mut self, delta: Vec2) {
        self.from += delta;
        self.to += delta;
    }
}

#[derive(Default)]
pub(super) struct Entrance {
    identity: Option<(u64, bool)>,
    started: f64,
}
impl Entrance {
    pub fn apply(&mut self, ui: &mut egui::Ui, identity: (u64, bool), reduced: bool) {
        let now = ui.input(|input| input.time);
        if self.identity != Some(identity) {
            self.identity = Some(identity);
            self.started = now;
        }
        if !reduced {
            let t = ((now - self.started) / f64::from(DURATION)).clamp(0.0, 1.0) as f32;
            if t < 1.0 {
                // The canvas stays opaque. Only the incoming content eases in;
                // no previous document or text is drawn underneath it.
                ui.multiply_opacity(0.88 + 0.12 * t * t * (3.0 - 2.0 * t));
                ui.ctx().request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_never_reverses_and_finishes_at_the_exact_target() {
        let motion = ScrollMotion::new(Vec2::new(0.0, 40.0), Vec2::new(0.0, 1040.0), 2.0);
        let mut previous = 40.0;
        for frame in 0..=32 {
            let (offset, _) = motion.sample(2.0 + f64::from(frame) * 0.005);
            assert!(offset.y >= previous && offset.y <= 1040.0);
            previous = offset.y;
        }
        assert_eq!(motion.sample(3.0), (Vec2::new(0.0, 1040.0), false));
        assert_eq!(motion.sample(1.0).0.y, 40.0);
    }
    #[test]
    fn a_fast_reverse_starts_at_the_visible_point_and_geometry_changes_shift_both_ends() {
        let forward = ScrollMotion::new(Vec2::ZERO, Vec2::new(0.0, 1000.0), 0.0);
        let visible = forward.sample(0.06).0;
        let mut back = ScrollMotion::new(visible, Vec2::ZERO, 0.06);
        assert_eq!(back.sample(0.06).0, visible);
        assert!(back.sample(0.08).0.y < visible.y);
        back.shift(Vec2::new(0.0, 300.0));
        assert_eq!(back.sample(1.0), (Vec2::new(0.0, 300.0), false));
    }
}
