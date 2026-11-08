#![forbid(unsafe_code)]
//! Experimental boot probe. Uses the ESP-IDF console and scheduler only.

#[cfg(not(all(
    esp_idf_esp_console_uart_default,
    esp_idf_esp_console_secondary_usb_serial_jtag
)))]
compile_error!(
    "Build boot-diagnostic with ESP_IDF_SDKCONFIG_DEFAULTS=sdkconfig.defaults.boot-diagnostic"
);

fn main() {
    esp_idf_sys::link_patches();
    let build = option_env!("GITHUB_SHA").unwrap_or("local");
    println!("StickShift boot diagnostic | Rust main entered | UART0 + USB Serial/JTAG | {build}");

    let mut sequence = 0u64;
    loop {
        println!("StickShift boot diagnostic | heartbeat {sequence}");
        sequence = sequence.wrapping_add(1);
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
