//! Backend-agnostic layout for the 135 x 240 StickS3 screen.
//! Only known values are drawn. Sensor and battery status aren't fabricated.

pub const WIDTH: i32 = 135;
pub const HEIGHT: i32 = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Color { Background, Accent, Primary, Muted, Warning }

pub trait Painter {
    type Error;
    fn clear(&mut self, color: Color) -> Result<(), Self::Error>;
    fn rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color)
        -> Result<(), Self::Error>;
    fn text(&mut self, x: i32, y: i32, message: &str, color: Color)
        -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health { Pending, Ready, Failed }
impl Health {
    fn label(self) -> &'static str {
        match self { Self::Pending=>"WAIT", Self::Ready=>"OK", Self::Failed=>"FAULT" }
    }
    fn color(self) -> Color {
        match self { Self::Ready=>Color::Accent, Self::Pending=>Color::Muted, Self::Failed=>Color::Warning }
    }
}

pub struct Snapshot<'a> {
    pub page: &'a str,
    pub imu: Health,
    pub power: Health,
    pub samples: u64,
    pub pitch: Option<f32>,
    pub roll: Option<f32>,
}

/// Small display layout. No heap allocation required.
pub fn render<P: Painter>(p: &mut P, s: &Snapshot<'_>) -> Result<(), P::Error> {
    p.clear(Color::Background)?;
    p.rect(0, 0, WIDTH as u32, 5, Color::Accent)?;
    p.text(8, 20, "STICKSHIFT", Color::Accent)?;
    p.text(8, 42, s.page, Color::Primary)?;
    p.rect(8, 60, 119, 1, Color::Muted)?;
    p.text(8, 78, "IMU", Color::Muted)?;
    p.text(60, 78, s.imu.label(), s.imu.color())?;
    p.text(8, 100, "POWER", Color::Muted)?;
    p.text(60, 100, s.power.label(), s.power.color())?;
    p.text(8, 122, "SAMPLES", Color::Muted)?;
    // Render numeric values only when samples are actually present.
    if s.samples > 0 { p.text(8, 140, "LIVE DATA", Color::Accent)?; }
    else { p.text(8, 140, "NO DATA", Color::Warning)?; }
    if let (Some(pitch), Some(roll)) = (s.pitch, s.roll) {
        if pitch.is_finite() && roll.is_finite() {
            p.text(8, 166, "TILT ACTIVE", Color::Accent)?;
        }
    }
    p.rect(8, 212, 119, 1, Color::Muted)?;
    p.text(8, 222, "A:NEXT  B:RESET", Color::Muted)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Trace { texts: Vec<String>, rectangles: Vec<(i32,i32,u32,u32)> }
    impl Painter for Trace {
        type Error=();
        fn clear(&mut self,_:Color)->Result<(),()> {Ok(())}
        fn rect(&mut self,x:i32,y:i32,w:u32,h:u32,_:Color)->Result<(),()> {
            self.rectangles.push((x,y,w,h)); Ok(()) }
        fn text(&mut self,_:i32,_:i32,s:&str,_:Color)->Result<(),()> {
            self.texts.push(s.into());Ok(())}
    }
    fn empty<'a>() -> Snapshot<'a> {
        Snapshot { page:"STATUS",imu:Health::Pending,power:Health::Ready,
            samples:0,pitch:None,roll:None }
    }
    #[test] fn unknown_sensors_never_render_as_ready() {
        let mut t=Trace::default();render(&mut t,&empty()).unwrap();
        assert!(t.texts.iter().any(|s|s=="NO DATA"));
        assert!(!t.texts.iter().any(|s|s=="LIVE DATA"));
    }
    #[test] fn nonfinite_tilt_is_hidden() {
        let mut t=Trace::default();let mut s=empty();s.pitch=Some(f32::NAN);s.roll=Some(0.);
        render(&mut t,&s).unwrap();
        assert!(!t.texts.iter().any(|s|s=="TILT ACTIVE"));
    }
    #[test] fn panel_elements_fit_screen() {
        let mut t=Trace::default();render(&mut t,&empty()).unwrap();
        assert!(t.rectangles.iter().all(|(x,y,w,h)|
            *x>=0 && *y>=0 && *x as u32+w<=WIDTH as u32 && *y as u32+h<=HEIGHT as u32));
    }
}
