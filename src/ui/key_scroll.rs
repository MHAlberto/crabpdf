//! Frame-driven arrow scrolling. OS repeat never determines animation cadence.
#[derive(Default)]
pub(super) struct KeyScroll {
    target: Option<f32>,
    direction: i8,
    held: f32,
    nudge: f32,
}
impl KeyScroll {
    pub fn cancel(&mut self) {
        *self = Self::default();
    }
    pub fn input(&mut self, direction: i8, tap: i8) {
        if direction != self.direction {
            self.held = 0.0;
            if direction != 0 {
                self.nudge += f32::from(direction) * 40.0;
            }
        } else if direction == 0 {
            self.nudge += f32::from(tap) * 40.0;
        }
        self.direction = direction;
    }
    pub fn nudge(&mut self, current: f32, delta: f32, max: f32) {
        self.target = Some((self.target.unwrap_or(current) + delta).clamp(0.0, max.max(0.0)));
    }
    pub fn shift(&mut self, delta: f32) {
        if let Some(target) = &mut self.target {
            *target += delta;
        }
    }
    pub fn active(&self) -> bool {
        self.target.is_some() || self.direction != 0 || self.nudge != 0.0
    }
    pub fn advance(&mut self, current: f32, max: f32, dt: f32, reduced: bool) -> Option<f32> {
        if !self.active() {
            return None;
        }
        // Cap an interrupted/occluded frame so resuming never jumps a whole screen.
        let dt = dt.clamp(0.0, 0.05);
        let before = self.held;
        self.held += dt;
        let held_dt = (self.held - 0.10).max(0.0) - (before - 0.10).max(0.0);
        let target = (self.target.unwrap_or(current)
            + self.nudge
            + f32::from(self.direction) * 650.0 * held_dt)
            .clamp(0.0, max.max(0.0));
        self.nudge = 0.0;
        let offset = if reduced {
            target
        } else {
            current + (target - current) * (1.0 - (-dt / 0.035).exp())
        };
        if (offset - target).abs() < 0.1 {
            self.target = None;
            Some(target)
        } else {
            self.target = Some(target);
            Some(offset)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_held_key_moves_between_repeat_events_and_stops_after_release() {
        let mut motion = KeyScroll::default();
        let mut y = 0.0;
        motion.input(1, 1);
        for frame in 0..60 {
            // Only the initial press: no synthetic OS repeats.
            motion.input(1, 0);
            let next = motion.advance(y, 2000.0, 1.0 / 120.0, false).unwrap();
            assert!(next > y, "stalled frame {frame}");
            assert!(next - y < 10.0);
            y = next;
        }
        assert!(y > 260.0 && y < 320.0);
        motion.input(0, 0);
        for _ in 0..80 {
            if let Some(next) = motion.advance(y, 2000.0, 1.0 / 120.0, false) {
                assert!(next >= y);
                y = next;
            }
        }
        assert!(!motion.active());
    }
    #[test]
    fn taps_reverse_bounds_focus_loss_and_reduced_motion_remain_predictable() {
        let mut motion = KeyScroll::default();
        motion.input(0, 1); // Press and release in the same native frame.
        let mut y = 100.0;
        for _ in 0..60 {
            y = motion.advance(y, 1000.0, 0.016, false).unwrap_or(y);
        }
        assert_eq!(y, 140.0);
        motion.input(-1, -1);
        assert!(motion.advance(y, 1000.0, 0.016, false).unwrap() < y);
        motion.cancel();
        assert!(!motion.active());
        motion.input(0, -1);
        assert_eq!(motion.advance(5.0, 1000.0, 10.0, true), Some(0.0));
        motion.input(0, 1);
        assert_eq!(motion.advance(995.0, 1000.0, 0.016, true), Some(1000.0));
    }
    #[test]
    fn cadence_is_independent_of_frame_rate_and_key_repeat() {
        fn run(hz: usize, repeats: bool) -> f32 {
            let mut motion = KeyScroll::default();
            let mut y = 0.0;
            for frame in 0..hz {
                motion.input(1, if repeats && frame % 3 == 0 { 1 } else { 0 });
                y = motion.advance(y, 5000.0, 1.0 / hz as f32, false).unwrap();
            }
            y
        }
        assert!((run(60, false) - run(120, true)).abs() < 5.0);
    }
}
