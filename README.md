# StickShift 🦀

Safe-Rust pocket sensor lab for **M5Stack StickS3 (K150)**. No expansion modules or network access.

## Honest status

**v0.1 is a board-bring-up starter, not a complete motion dashboard.** Firmware uses onboard buttons, attempts read-only BMI270 and ES8311 identification on the I2C bus, and emits a USB serial heartbeat. Application logic for motion, tilt, thresholds, page navigation and debounce has tests but is not yet connected to live IMU samples. The default build does **not** configure/read BMI270 motion, activate LCD power, capture audio, decode IR, or estimate battery state. The gated LCD preview is experimental and unverified on target. Both ESP32-S3 feature configurations have been linked as Xtensa ELF executables in CI; neither has been flashed or tested on physical hardware.

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

The M5PM1 driver in `src/pmic.rs` provides identity-checked, read-modify-write, readback-verified L3B GPIO2 level control using only GPIO2 fields. The caller must explicitly choose the output level. It is called only in the explicitly enabled LCD preview. The default serial-only firmware makes no power-rail changes. Do not substitute undocumented whole-register writes.

The display layout in `src/dashboard.rs` is a hardware-independent, allocation-free command layer. The `src/st7789.rs` renderer now implements ST7789P3 initialization commands, fixed 135×240 offsets (52,40), clipped rectangles, streamed RGB565 fills and an 8×8 text renderer over a safe `DisplayIo` trait. These are **host-tested protocol routines**. A guarded ESP-IDF SPI transport is implemented behind the optional `lcd-preview` feature and both firmware feature sets have passed the ESP32-S3 `cargo check` and ELF linking gates, but neither has been electrically validated. It explicitly shows `NO DATA` until sensor sampling works. It is **not yet displayed on hardware**. The BMI270 Bosch configuration initialization remains a separate milestone.

Register source: https://github.com/m5stack/M5PM1/tree/main/src ; board wiring: https://docs.m5stack.com/en/core/StickS3

The PMIC's documented device-ID register (`0x00`) normally reports `0x6E`; register `0x01` is a *separate* device-model value, not a combined 16-bit ID. StickShift validates only the documented ID and masks GPIO2 writes.

**Vendor LCD polarity:** M5GFX's StickS3-specific `sticks3_power_on` sets M5PM1 GPIO2 HIGH by setting bit `0x04` in register `0x11`. The preview uses this polarity explicitly; no automatic PMIC writes in the default build. On-device behavior is still unverified. Reference: https://github.com/m5stack/M5GFX/blob/master/src/board_detect/m5/pmic_ops.hpp

### Opt-in LCD integration (experimental, do not flash without validation)

A normal firmware build still uses USB serial only. The LCD integration is compiled only with `lcd-preview` and exactly one of `lcd-l3b-low` or `lcd-l3b-high`. These options deliberately have no default. Selecting one writes the M5PM1 PYG2 L3B power gate and then uses SPI2 (G40 clock, G39 MOSI, G41 CS, G45 DC), display reset G21, and backlight G38. The experiment requires the board's actual rail polarity and LCD reset sequence to be independently validated. The preview has passed ESP32-S3 target `cargo check` and ELF linking, but has no physical-test evidence.

Example compile-only selection (not a flash command):

```sh
cargo +esp build --target xtensa-esp32s3-espidf --no-default-features --features firmware,lcd-preview,lcd-l3b-high
```

StickS3 M5GFX uses HIGH for the display rail; retain explicit opt-in until target compilation and physical verification are complete.

## CI firmware artifacts (not automatically flashed)

The ESP32-S3 CI builds both `firmware` (serial-only) and `firmware,lcd-preview,lcd-l3b-high` (guarded display preview) as linked ELF firmware images. If the complete workflow passes, its GitHub Actions **Artifacts** section includes `serial/stickshift.elf` and `lcd-preview/stickshift.elf`, the resolved `Cargo.lock`, SHA-256 checksums, and `BUILD.txt` recording the exact commit and build configuration. Download via the corresponding workflow run.

The ELFs are suitable as inputs to an ESP-IDF-compatible flashing tool such as `espflash`, but **no flash command is run by CI**, and neither image has yet been tested on the physical StickS3. The LCD-preview version changes the PMIC power rail, so do not select or flash it without an explicit hardware bring-up decision. A successful cross-build alone does not establish device safety or correct display operation.

## Serial-only hardware bring-up gate

**Do not use the LCD-preview ELF for the first flash.** It intentionally changes the M5PM1 GPIO2/L3B power rail; serial-only firmware makes no PMIC register writes. Physical flashing is a separate, explicitly approved operator action, not a consequence of fetching this repository or its CI artifacts.

Before any approved device write:

1. Identify the actual StickS3 board, ESP32-S3 USB interface, available serial port, power source and whether existing firmware/data must be preserved. Record the board and existing state. Do not assume that an ELF is a complete factory image or that the partition table, bootloader and application offsets have been validated.
2. Read the CI archive's `BUILD.txt` and `SHA256SUMS`, verify all hashes, and select **only** `serial/stickshift.elf`. Do not use `lcd-preview/stickshift.elf` in this phase. Confirm the build's feature selection is `firmware` alone.
3. Use the ESP-IDF-compatible tooling for the device and verify the generated flash image, offsets, partition map and bootloader compatibility **before** issuing a flash command. The archived ELF by itself is not a merged `.bin` image and this document deliberately does not prescribe a destructive flash command.
4. With explicit approval and an established recovery procedure, flash only the reviewed serial configuration. Capture port/device identity, tool version, exact image hashes, flash addresses, serial output and any errors.
5. On boot, look for the `StickShift | StickS3 | USB serial` banner, BMI270 and ES8311 read-only identification attempts, and periodic `alive | ... | no live IMU` heartbeats. Exercise button A (page cycle) and B (reset statistics). Record actual observations rather than inferring success from the build.
6. Stop on reset loops, unexpected PMIC activity, missing communications, abnormal heating or unexplained hardware behavior. Preserve logs and evidence. Do not proceed to LCD-preview without a separate review of the vendor power-on sequence and a new explicit approval.

**Evidence status:** the source PR head was `15e13244f6f968f04aa7f5c3c5e44801a8239180` when the archived firmware passed CI. The artifact's `BUILD.txt` records GitHub's PR merge-tree SHA `24cb41e25cf97cb1a715297ae7e870526681d391`. Keep both identifiers with any retained archive. The verified CI ZIP checksum is `c1d7367e7349b92e711a3bbcc7f0430a072dba008b426ec9de785fbb0668064f`. This historical firmware artifact does **not** establish validation of any later source or documentation commit.

Retained copy of that artifact: [Techbenders / Projects / StickShift](https://drive.google.com/drive/folders/1rl-gwIEKAPRFpJTEJuqAy67IxErjHk2u). Preserve checksum and original build manifest with it.

## Experimental boot diagnostic

`boot-diagnostic` is a separate ESP32-S3 binary for investigating startup and USB
enumeration. It prints a Rust entry marker and a numbered heartbeat through the
ESP-IDF console, with UART0 at 115200 baud as the primary output and USB
Serial/JTAG as the secondary output. It leaves buttons, I²C, display, PMIC,
radio, and storage out of the application.

Build without accessing the board:

```sh
ESP_IDF_SDKCONFIG_DEFAULTS=sdkconfig.defaults.boot-diagnostic \
ESP_IDF_SDKCONFIG=sdkconfig.boot-diagnostic \
cargo +esp build --target xtensa-esp32s3-espidf --no-default-features \
  --features boot-diagnostic --bin boot-diagnostic
```

CI includes the diagnostic ELF and its defaults in the firmware archive. The
diagnostic is unverified on physical hardware. Capture a normal-boot UART0
trace from the installed image before considering a separately authorized
diagnostic flash.

## Host tests

```sh
cargo test --lib --no-default-features
```

## Firmware on macOS / Linux

Set up the Espressif Rust toolchain, ESP-IDF, `ldproxy`, and `espflash` using the ESP-RS documentation. Ordinary upstream Rust does not support the Xtensa ESP32-S3 firmware target.

```sh
cargo +esp build --features firmware --bin stickshift --target xtensa-esp32s3-espidf
```

Flashing is a separate action and is **not** performed by CI or these documentation steps. Both serial-only and LCD-preview configurations passed `cargo check` and produced linked Xtensa ELFs in GitHub Actions run [42435308217](https://github.com/danielbrownjr/stickshift/actions/runs/42435308217). No device flash or physical validation has occurred.

## Increments

1. v0.1: USB serial, buttons, read-only I2C identification (starter only; hardware unverified)
2. v0.2: Guarded PMIC driver, dashboard and ST7789 protocol renderer (host-tested, ESP-IDF SPI transport and physical panel not yet validated)
3. v0.3: BMI270 Bosch configuration, acceleration, gyro and motion dashboard
4. v0.4: ES8311 audio meter and IR activity
5. v0.5: validated battery telemetry and low-power modes

Application-owned code forbids unsafe Rust. ESP-IDF, HAL and C dependencies can contain unsafe internally; this is not a claim of end-to-end memory safety.
