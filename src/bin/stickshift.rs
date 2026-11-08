#![forbid(unsafe_code)]
//! Initial on-device bring-up: buttons and onboard I2C identity checks over USB serial.
//! Motion samples, LCD, sound and IR decoding are deliberately not claimed in v0.1.
use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{PinDriver, Pull},
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    units::Hertz,
};
use stickshift::{App, Debouncer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let peripherals = Peripherals::take()?;
    let pins = peripherals.pins;
    let mut a = PinDriver::input(pins.gpio11)?;
    let mut b = PinDriver::input(pins.gpio12)?;
    a.set_pull(Pull::Up)?;
    b.set_pull(Pull::Up)?;
    let mut i2c = I2cDriver::new(peripherals.i2c0, pins.gpio47, pins.gpio48,
        &I2cConfig::new().baudrate(Hertz(100_000)))?;
    let mut app = App::default();
    let mut da = Debouncer::default();
    let mut db = Debouncer::default();
    println!("StickShift 0.1 | StickS3 | USB serial diagnostics");
    // Registers read only. Other M5PM1 registers must not be casually written.
    for (label, addr, reg) in [("BMI270 chip ID", 0x68, 0x00),
        ("ES8311 ID register", 0x18, 0xFD)] {
        let mut byte = [0u8; 1];
        match i2c.write_read(addr, &[reg], &mut byte, 1000) {
            Ok(()) => println!("{label}: 0x{:02X} at 0x{addr:02X}", byte[0]),
            Err(e) => println!("{label}: unavailable ({e:?})"),
        }
    }
    println!("M5PM1 0x6E (power controller) not probed with undocumented registers");
    println!("A: cycle page, B: reset peaks; reports every second");
    let mut ticks: u32 = 0;
    loop {
        if da.sample(a.is_low()) { app.next_page(); println!("page: {}", app.page.label()); }
        if db.sample(b.is_low()) { app.reset_peaks(); println!("stats reset"); }
        ticks += 1;
        if ticks >= 50 { ticks = 0; println!("alive | page={} | samples={} | motion not initialized", app.page.label(), app.samples); }
        FreeRtos::delay_ms(20);
    }
}
