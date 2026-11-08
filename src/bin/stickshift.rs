#![forbid(unsafe_code)]
//! StickS3 bring-up. The LCD path is opt-in pending board validation.
use esp_idf_hal::{
    delay::FreeRtos,
    gpio::{PinDriver, Pull},
    i2c::{I2cConfig, I2cDriver},
    peripherals::Peripherals,
    units::Hertz,
};
use stickshift::{App, Debouncer};

#[cfg(all(feature="lcd-preview", not(any(feature="lcd-l3b-high", feature="lcd-l3b-low"))))]
compile_error!("lcd-preview requires exactly one L3B polarity feature");
#[cfg(all(feature="lcd-l3b-high", feature="lcd-l3b-low"))]
compile_error!("select only one L3B polarity");

#[cfg(feature="lcd-preview")]
use esp_idf_hal::{
    gpio::AnyInputPin,
    spi::{config, SpiDeviceDriver, SpiDriverConfig},
    units::FromValueType,
};
#[cfg(feature="lcd-preview")]
use embedded_hal::{digital::OutputPin, spi::{SpiDevice, MODE_3}};
#[cfg(feature="lcd-preview")]
use stickshift::{
    dashboard::{render, Health, Snapshot},
    pmic::{self, RegisterBus},
    st7789::{DisplayIo, Panel},
};

#[cfg(feature="lcd-preview")]
struct RegisterAdapter<'a, 'd>(&'a mut I2cDriver<'d>);

#[cfg(feature="lcd-preview")]
impl RegisterBus for RegisterAdapter<'_, '_> {
    type Error=esp_idf_sys::EspError;
    fn read(&mut self, register:u8)->Result<u8,Self::Error> {
        let mut data=[0u8;1];
        self.0.write_read(pmic::ADDRESS,&[register],&mut data,1000)?;
        Ok(data[0])
    }
    fn write(&mut self,register:u8,value:u8)->Result<(),Self::Error> {
        self.0.write(pmic::ADDRESS,&[register,value],1000)
    }
}

#[cfg(feature="lcd-preview")]
struct SpiDisplayIo<S,D> { spi:S, dc:D }

#[cfg(feature="lcd-preview")]
impl<S,D> DisplayIo for SpiDisplayIo<S,D>
where S:SpiDevice<u8>, D:OutputPin
{
    type Error=String;
    fn command(&mut self, cmd:u8, args:&[u8]) -> Result<(),Self::Error> {
        self.dc.set_low().map_err(|e| format!("{e:?}"))?;
        self.spi.write(&[cmd]).map_err(|e| format!("{e:?}"))?;
        if !args.is_empty() {
            self.dc.set_high().map_err(|e| format!("{e:?}"))?;
            self.spi.write(args).map_err(|e| format!("{e:?}"))?;
        }
        Ok(())
    }
    fn data(&mut self, data:&[u8]) -> Result<(),Self::Error> {
        self.dc.set_high().map_err(|e| format!("{e:?}"))?;
        self.spi.write(data).map_err(|e| format!("{e:?}"))
    }
    fn delay_ms(&mut self,ms:u32) { FreeRtos::delay_ms(ms); }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();
    let peripherals=Peripherals::take()?;
    let pins=peripherals.pins;

    let a=PinDriver::input(pins.gpio11,Pull::Up)?;
    let b=PinDriver::input(pins.gpio12,Pull::Up)?;
    let mut i2c=I2cDriver::new(peripherals.i2c0,pins.gpio47,pins.gpio48,
        &I2cConfig::new().baudrate(Hertz(100_000)))?;
    let mut app=App::default();
    let mut da=Debouncer::default();
    let mut db=Debouncer::default();

    println!("StickShift | StickS3 | USB serial");
    for (label,addr,reg) in [("BMI270 chip ID",0x68,0x00),
        ("ES8311 register",0x18,0xfd)] {
        let mut byte=[0u8;1];
        match i2c.write_read(addr,&[reg],&mut byte,1000) {
            Ok(())=>println!("{label}: 0x{:02X}",byte[0]),
            Err(e)=>println!("{label}: unavailable ({e:?})"),
        }
    }

    // No PMIC writes or display operations in the normal serial-only build.
    #[cfg(feature="lcd-preview")]
    let mut backlight=PinDriver::output(pins.gpio38)?;
    #[cfg(feature="lcd-preview")]
    let mut reset=PinDriver::output(pins.gpio21)?;
    #[cfg(feature="lcd-preview")]
    backlight.set_low()?;
    #[cfg(feature="lcd-preview")]
    reset.set_low()?;

    // Physical LCD route requires an explicit build-time selection of rail polarity.
    #[cfg(feature="lcd-preview")]
    let mut panel={
        let level_high=cfg!(feature="lcd-l3b-high");
        println!("LCD preview requested; PYG2 high={level_high}");
        pmic::set_l3b_level(&mut RegisterAdapter(&mut i2c),level_high)
            .map_err(|e| format!("M5PM1 refused rail change: {e:?}"))?;
        FreeRtos::delay_ms(20);
        reset.set_high()?;
        FreeRtos::delay_ms(120);
        let spi_cfg=config::Config::new().baudrate(26.MHz().into()).data_mode(MODE_3);
        let device=SpiDeviceDriver::new_single(peripherals.spi2,pins.gpio40,pins.gpio39,
            Option::<AnyInputPin>::None,Some(pins.gpio41),
            &SpiDriverConfig::new(),&spi_cfg)?;
        let dc=PinDriver::output(pins.gpio45)?;
        let io=SpiDisplayIo{spi:device,dc};
        let mut panel=Panel::new(io);
        panel.initialize().map_err(|e| format!("ST7789 init failed: {e:?}"))?;
        panel
    };

    #[cfg(feature="lcd-preview")]
    {
        backlight.set_high()?;
        render(&mut panel,&Snapshot{page:app.page.label(),imu:Health::Pending,
            power:Health::Ready,samples:app.samples,pitch:None,roll:None})
            .map_err(|e| format!("LCD drawing failed: {e:?}"))?;
    }

    println!("A: next page, B: reset; IMU readings unavailable until Bosch init");
    let mut ticks=0u32;
    loop {
        if da.sample(a.is_low()) {
            app.next_page();println!("page: {}",app.page.label());
            #[cfg(feature="lcd-preview")]
            render(&mut panel,&Snapshot{page:app.page.label(),imu:Health::Pending,
                power:Health::Ready,samples:app.samples,pitch:None,roll:None})
                .map_err(|e| format!("LCD drawing failed: {e:?}"))?;
        }
        if db.sample(b.is_low()) { app.reset_peaks();println!("stats reset"); }
        ticks+=1;
        if ticks>=50 {ticks=0;println!("alive | page={} | samples={} | no live IMU",
            app.page.label(),app.samples);}
        FreeRtos::delay_ms(20);
    }
}
