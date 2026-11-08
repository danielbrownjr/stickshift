# StickShift 🦀

Safe-Rust pocket sensor lab for **M5Stack StickS3 (K150)**. No expansion modules or network access.

## Honest status

**v0.1 is a board-bring-up starter, not a complete motion dashboard.** Firmware uses onboard buttons, attempts read-only BMI270 and ES8311 identification on the I2C bus, and emits a USB serial heartbeat. Application logic for motion, tilt, thresholds, page navigation and debounce has tests but is not yet connected to live IMU samples. The default build does **not** configure/read BMI270 motion, activate LCD power, capture audio, decode IR, or estimate battery state. The gated LCD preview is experimental and unverified on target. This project has not been built or tested on physical hardware.

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

The M5PM1 driver in `src/pmic.rs` provides identity-checked, read-modify-write, readback-verified L3B GPIO2 level control using only GPIO2 fields. The caller must explicitly choose the output level. It is **not called by firmware yet**, because physical verification of the PMIC output polarity and the full display path is still required. Do not substitute undocumented whole-register writes.

The display layout in `src/dashboard.rs` is a hardware-independent, allocation-free command layer. The `src/st7789.rs` renderer now implements ST7789P3 initialization commands, fixed 135×240 offsets (52,40), clipped rectangles, streamed RGB565 fills and an 8×8 text renderer over a safe `DisplayIo` trait. These are **host-tested protocol routines**. A guarded ESP-IDF SPI transport is implemented behind the optional `lcd-preview` feature, but has **not** been cross-compiled or electrically validated. It explicitly shows `NO DATA` until sensor sampling works. It is **not yet displayed on hardware**. The BMI270 Bosch configuration initialization remains a separate milestone.

Register source: https://github.com/m5stack/M5PM1/tree/main/src ; board wiring: https://docs.m5stack.com/en/core/StickS3

**Power polarity hold:** M5Stack's L3B API example drives GPIO2 low, while a separate hardware-verified Zephyr port reports high. The PMIC helper is invoked **only** when the opt-in LCD preview is compiled. Verify the correct rail polarity from the board schematic and hardware before selecting a preview feature. Do not interpret host tests as permission to flash this power-control path.

### Opt-in LCD integration (experimental, do not flash without validation)

A normal firmware build still uses USB serial only. The LCD integration is compiled only with `lcd-preview` and exactly one of `lcd-l3b-low` or `lcd-l3b-high`. These options deliberately have no default. Selecting one writes the M5PM1 PYG2 L3B power gate and then uses SPI2 (G40 clock, G39 MOSI, G41 CS, G45 DC), display reset G21, and backlight G38. The experiment requires the board's actual rail polarity and LCD reset sequence to be independently validated. The preview currently has no target-build or physical-test evidence.

Example compile-only selection (not a flash command):

```sh
cargo +esp build --target xtensa-esp32s3-espidf --no-default-features --features firmware,lcd-preview,lcd-l3b-high
```

StickS3 M5GFX uses HIGH for the display rail; retain explicit opt-in until target compilation and physical verification are complete.

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
2. v0.2: Guarded PMIC driver, dashboard and ST7789 protocol renderer (host-tested, ESP-IDF SPI transport and physical panel not yet validated)
3. v0.3: BMI270 Bosch configuration, acceleration, gyro and motion dashboard
4. v0.4: ES8311 audio meter and IR activity
5. v0.5: validated battery telemetry and low-power modes

Application-owned code forbids unsafe Rust. ESP-IDF, HAL and C dependencies can contain unsafe internally; this is not a claim of end-to-end memory safety.
