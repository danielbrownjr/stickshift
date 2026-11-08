fn main() {
    println!("cargo:rustc-check-cfg=cfg(esp_idf_esp_console_uart_default)");
    println!("cargo:rustc-check-cfg=cfg(esp_idf_esp_console_secondary_usb_serial_jtag)");
    if std::env::var_os("CARGO_FEATURE_FIRMWARE").is_some()
        || std::env::var_os("CARGO_FEATURE_BOOT_DIAGNOSTIC").is_some()
    {
        embuild::espidf::sysenv::output();
    }
}
