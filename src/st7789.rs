//! ST7789P3 panel protocol and a tiny no-framebuffer dashboard renderer.
//! The 135x240 panel occupies an offset window on ST7789 RAM.
//! Bus transport is injected and testable without an ESP-IDF toolchain.
use crate::dashboard::{Color, Painter};

pub const WIDTH: u16 = 135;
pub const HEIGHT: u16 = 240;
pub const X_OFFSET: u16 = 52;
pub const Y_OFFSET: u16 = 40;

pub trait DisplayIo {
    type Error;
    fn command(&mut self, cmd: u8, args: &[u8]) -> Result<(), Self::Error>;
    fn data(&mut self, data: &[u8]) -> Result<(), Self::Error>;
    fn delay_ms(&mut self, ms: u32);
}

#[derive(Debug, PartialEq, Eq)]
pub enum DisplayError<E> {
    Transport(E),
    InvalidWindow,
}

pub struct Panel<I> { io: I }

impl<I: DisplayIo> Panel<I> {
    pub fn new(io: I) -> Self { Self { io } }

    /// Call only after external reset and power rail are stable.
    pub fn initialize(&mut self) -> Result<(), DisplayError<I::Error>> {
        self.cmd(0x01, &[])?; // SWRESET
        self.io.delay_ms(150);
        self.cmd(0x11, &[])?; // SLPOUT
        self.io.delay_ms(120);
        self.cmd(0x3a, &[0x55])?; // 16-bit RGB565
        self.cmd(0x36, &[0x08])?; // BGR; portrait orientation
        self.cmd(0x21, &[])?; // INVON, required by this panel
        self.cmd(0x13, &[])?; // NORON
        self.io.delay_ms(10);
        self.cmd(0x29, &[])?; // DISPON
        self.io.delay_ms(20);
        Ok(())
    }

    fn cmd(&mut self, cmd: u8, args: &[u8]) -> Result<(), DisplayError<I::Error>> {
        self.io.command(cmd, args).map_err(DisplayError::Transport)
    }

    /// Clamped logical coordinates. Never wraps outside the visible window.
    fn window(&mut self, x: u16, y: u16, w: u16, h: u16)
        -> Result<(), DisplayError<I::Error>>
    {
        if w == 0 || h == 0 || x >= WIDTH || y >= HEIGHT ||
            u32::from(x) + u32::from(w) > u32::from(WIDTH) ||
            u32::from(y) + u32::from(h) > u32::from(HEIGHT) {
            return Err(DisplayError::InvalidWindow);
        }
        let x0 = x + X_OFFSET;
        let x1 = x0 + w - 1;
        let y0 = y + Y_OFFSET;
        let y1 = y0 + h - 1;
        self.cmd(0x2a, &[(x0 >> 8) as u8, x0 as u8, (x1 >> 8) as u8, x1 as u8])?;
        self.cmd(0x2b, &[(y0 >> 8) as u8, y0 as u8, (y1 >> 8) as u8, y1 as u8])?;
        self.cmd(0x2c, &[])
    }

    /// Streaming RGB565, 64 pixels per transfer; no full-frame allocation.
    pub fn fill(&mut self, x: i32, y: i32, w: u32, h: u32, rgb565: u16)
        -> Result<(), DisplayError<I::Error>>
    {
        if w == 0 || h == 0 { return Ok(()); }
        let right = i64::from(x) + i64::from(w);
        let bottom = i64::from(y) + i64::from(h);
        let x0 = i64::from(x).max(0).min(i64::from(WIDTH));
        let y0 = i64::from(y).max(0).min(i64::from(HEIGHT));
        let x1 = right.max(0).min(i64::from(WIDTH));
        let y1 = bottom.max(0).min(i64::from(HEIGHT));
        if x1 <= x0 || y1 <= y0 { return Ok(()); }
        let width = (x1-x0) as u16;
        let height = (y1-y0) as u16;
        self.window(x0 as u16, y0 as u16, width, height)?;
        let [hi,lo] = rgb565.to_be_bytes();
        let mut buffer = [0u8; 128];
        for i in 0..64 { buffer[i*2] = hi; buffer[i*2+1] = lo; }
        let mut remaining = usize::from(width) * usize::from(height);
        while remaining > 0 {
            let n = remaining.min(64);
            self.io.data(&buffer[..n*2]).map_err(DisplayError::Transport)?;
            remaining -= n;
        }
        Ok(())
    }

    pub fn text8(&mut self, x: i32, y: i32, message: &str, rgb565: u16)
        -> Result<(), DisplayError<I::Error>>
    {
        use font8x8::{UnicodeFonts, BASIC_FONTS};
        for (index, c) in message.chars().take(16).enumerate() {
            let pos = x + index as i32 * 8;
            if let Some(glyph) = BASIC_FONTS.get(c) {
                for (row, bits) in glyph.iter().enumerate() {
                    // Coalesce adjacent pixels to minimize SPI transactions.
                    let mut start: Option<usize> = None;
                    for col in 0..=8 {
                        let on = col < 8 && bits & (1 << col) != 0;
                        if on && start.is_none() { start = Some(col); }
                        if !on {
                            if let Some(left) = start.take() {
                                self.fill(pos + left as i32, y + row as i32,
                                    (col-left) as u32, 1, rgb565)?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

fn palette(c: Color) -> u16 {
    match c {
        Color::Background => 0x0000,
        Color::Accent => 0x07e0,
        Color::Primary => 0xffff,
        Color::Muted => 0x8410,
        Color::Warning => 0xfbe0,
    }
}

impl<I: DisplayIo> Painter for Panel<I> {
    type Error = DisplayError<I::Error>;
    fn clear(&mut self, color: Color) -> Result<(), Self::Error> {
        self.fill(0, 0, u32::from(WIDTH), u32::from(HEIGHT), palette(color))
    }
    fn rect(&mut self, x: i32, y: i32, w: u32, h: u32, color: Color)
        -> Result<(), Self::Error> {
        self.fill(x, y, w, h, palette(color))
    }
    fn text(&mut self, x: i32, y: i32, value: &str, color: Color)
        -> Result<(), Self::Error> {
        self.text8(x, y, value, palette(color))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Default)]
    struct Trace { ops: Vec<(u8, Vec<u8>)>, bytes: usize, waits: Vec<u32> }
    impl DisplayIo for Trace {
        type Error = ();
        fn command(&mut self, cmd:u8, args:&[u8]) -> Result<(), ()> {
            self.ops.push((cmd,args.to_vec()));Ok(())
        }
        fn data(&mut self, data:&[u8]) -> Result<(), ()> {
            self.bytes+=data.len();Ok(())
        }
        fn delay_ms(&mut self,ms:u32) { self.waits.push(ms); }
    }
    #[test] fn initialization_has_sleep_out_and_display_on() {
        let mut panel=Panel::new(Trace::default());panel.initialize().unwrap();
        let cs:Vec<u8>=panel.io.ops.iter().map(|(c,_)|*c).collect();
        assert_eq!(cs,vec![0x01,0x11,0x3a,0x36,0x21,0x13,0x29]);
        assert_eq!(panel.io.waits,vec![150,120,10,20]);
    }
    #[test] fn addressing_adds_panel_offsets() {
        let mut panel=Panel::new(Trace::default());panel.fill(0,0,2,3,0x1234).unwrap();
        assert_eq!(panel.io.ops[0],(0x2a,vec![0,52,0,53]));
        assert_eq!(panel.io.ops[1],(0x2b,vec![0,40,0,42]));
        assert_eq!(panel.io.bytes,12);
    }
    #[test] fn fill_clips_and_does_not_wrap() {
        let mut panel=Panel::new(Trace::default());
        panel.fill(134,239,1000,1000,0x1234).unwrap();
        assert_eq!(panel.io.bytes,2);
        panel.fill(-20,-20,2,2,0x1234).unwrap();
        assert_eq!(panel.io.bytes,2);
        panel.fill(135,0,1,1,0x1234).unwrap();
        assert_eq!(panel.io.bytes,2);
    }
    #[test] fn full_clear_is_bounded_streaming() {
        let mut panel=Panel::new(Trace::default());panel.clear(Color::Background).unwrap();
        assert_eq!(panel.io.bytes,135*240*2);
    }
}
