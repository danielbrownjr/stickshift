# StickShift 🦀

Safe-Rust pocket sensor lab for **M5Stack StickS3 (K150)**. No expansion modules or network access.

## Honest status

**v0.1 is a board-bring-up starter, not a complete motion dashboard.** Firmware uses onboard buttons, attempts read-only BMI270 and ES8311 identification on the I2C bus, and emits a USB serial heartbeat. Application logic for motion, tilt, thresholds, page navigation and debounce has tests but is not yet connected to live IMU samples. It does **not yet** configure/read BMI270 motion, activate LCD power, capture audio, decode IR, or estimate battery state. This project has not been built or tested on physical hardware.

## Hardware map

| Peripheral | Connection |
|---|---|
| Buttons A / B | GPIO 11 / 12 (active low) |
| BMI270 IMU | I2C `0x68`, SDA 47, SCL 48 |
| M5PM1 power controller | I2C `0x6E`, SDA 47, SCL 48 |
| ES8311 codec | I2C `0x18`, SDA 47, SCL 48 |
| ST7789P3 LCD | SPI MOSI 39, SCK 40, DC 45, CS 41, RESET 21, BL 38 |
| IR | TX 46, RX 42 |

Source: https://docs.m5stack.com/en/core/StickS3

**LCD caution:** its power domain is managed by M5PM1. Validate M5PM1 initialization before enabling the display. **BMI270 caution:** raw readout without the Bosch configuration upload is invalid; chip-ID success does not mean the IMU is initialized.

## L3B and screen bring-up design

The M5PM1 driver in `src/pmic.rs` provides an identity-checked, read-modify-write, readback-verified L3B enable sequence using only GPIO2 fields. It is **not called by firmware yet**, because physical verification of the PMIC output polarity and the full display path is still required. Do not substitute undocumented whole-register writes.

The display layout in `src/dashboard.rs` is a hardware-independent, allocation-free command layer for a future ST7789 renderer. It explicitly shows `NO DATA` until sensor sampling works. It is **not yet displayed on hardware**. The BMI270 Bosch configuration initialization remains a separate milestone.

Register source: https://github.com/m5stack/M5PM1/tree/main/src ; board wiring: https://docs.m5stack.com/en/core/StickS3

## Host tests

```sh
cargo test --lib --no-default-features
```

## Firmware on macOS / Linux

Set up the Espressif Rust toolchain, ESP-IDF, `ldproxy`, and `espflash` using the ESP-RS documentation. Ordinary upstream Rust does not support the Xtensa ESP32-S3 firmware target.

```sh
cargo +esp build --features firmware --bin stickshift --target xtensa-esp32s3-espidf
```

Flashing is a separate action and is **not** performed by this project import. Dependency versions have not been cross-compiled or validated.

## Increments

1. v0.1: USB serial, buttons, read-only I2C identification (starter only; hardware unverified)
2. v0.2: L3B safe PMIC driver and dashboard layout (implemented and unit-tested in host CI; not yet wired to firmware), followed by ST7789 SPI integration
3. v0.3: BMI270 Bosch configuration, acceleration, gyro and motion dashboard
4. v0.4: ES8311 audio meter and IR activity
5. v0.5: validated battery telemetry and low-power modes

Application-owned code forbids unsafe Rust. ESP-IDF, HAL and C dependencies can contain unsafe internally; this is not a claim of end-to-end memory safety.
