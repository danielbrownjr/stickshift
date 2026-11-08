#![forbid(unsafe_code)]
//! Board-independent sensing and UI state. No hardware or IDF coupling.

pub mod pmic;
pub mod dashboard;
pub mod st7789;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    /// Acceleration in multiples of standard gravity.
    pub accel_g: [f32; 3],
    /// Angular rate in degrees per second.
    pub gyro_dps: [f32; 3],
}

impl Motion {
    pub fn magnitude_g(self) -> f32 {
        self.accel_g.iter().map(|v| v * v).sum::<f32>().sqrt()
    }
    pub fn tilt_degrees(self) -> (f32, f32) {
        let [x, y, z] = self.accel_g;
        let pitch = (-x).atan2((y * y + z * z).sqrt()).to_degrees();
        let roll = y.atan2(z).to_degrees();
        (pitch, roll)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page { Status, Motion, Sound, Ir }
impl Page {
    pub fn next(self) -> Self {
        match self { Self::Status => Self::Motion, Self::Motion => Self::Sound,
            Self::Sound => Self::Ir, Self::Ir => Self::Status }
    }
    pub fn label(self) -> &'static str {
        match self { Self::Status => "STATUS", Self::Motion => "MOTION",
            Self::Sound => "SOUND", Self::Ir => "IR" }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Debouncer { stable: bool, last_sample: bool, count: u8 }
impl Debouncer {
    /// Returns true on a debounced press edge; call at a fixed cadence.
    pub fn sample(&mut self, pressed: bool) -> bool {
        if pressed == self.last_sample { self.count = self.count.saturating_add(1); }
        else { self.last_sample = pressed; self.count = 1; }
        if self.count >= 3 && self.stable != pressed {
            self.stable = pressed;
            return pressed;
        }
        false
    }
}

#[derive(Clone, Copy, Debug)]
pub struct App {
    pub page: Page,
    pub samples: u64,
    pub last_motion: Option<Motion>,
    pub peak_g: f32,
    pub shake_events: u32,
    above_threshold: bool,
}
impl Default for App {
    fn default() -> Self { Self { page: Page::Status, samples: 0, last_motion: None,
        peak_g: 0.0, shake_events: 0, above_threshold: false } }
}
impl App {
    pub fn next_page(&mut self) { self.page = self.page.next(); }
    pub fn reset_peaks(&mut self) { self.peak_g = 0.0; self.shake_events = 0; self.above_threshold = false; }
    pub fn ingest_motion(&mut self, motion: Motion) {
        let mag = motion.magnitude_g();
        if mag.is_finite() {
            self.samples = self.samples.saturating_add(1);
            self.last_motion = Some(motion);
            self.peak_g = self.peak_g.max(mag);
            // Hysteresis, not a calibrated impact or safety detector.
            if mag > 1.8 && !self.above_threshold {
                self.shake_events = self.shake_events.saturating_add(1);
                self.above_threshold = true;
            } else if mag < 1.3 { self.above_threshold = false; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn pages_wrap() { let mut p = Page::Status; for _ in 0..4 { p = p.next(); } assert_eq!(p, Page::Status); }
    #[test] fn tilt_flat() { let (p, r) = Motion { accel_g: [0., 0., 1.], ..Default::default() }.tilt_degrees(); assert!(p.abs() < 0.01 && r.abs() < 0.01); }
    #[test] fn debouncer_one_edge() { let mut d = Debouncer::default(); assert!(!d.sample(true)); assert!(!d.sample(true)); assert!(d.sample(true)); assert!(!d.sample(true)); }
    #[test] fn shake_hysteresis() { let mut a = App::default(); let m = |z| Motion { accel_g: [0., 0., z], ..Default::default() }; a.ingest_motion(m(2.)); a.ingest_motion(m(2.)); assert_eq!(a.shake_events, 1); a.ingest_motion(m(1.)); a.ingest_motion(m(2.)); assert_eq!(a.shake_events, 2); }
    #[test] fn reject_nonfinite() { let mut a = App::default(); a.ingest_motion(Motion { accel_g: [f32::NAN, 0., 0.], ..Default::default() }); assert_eq!(a.samples, 0); }
}
