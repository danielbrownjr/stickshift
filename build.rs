fn main() {
    if std::env::var_os("CARGO_FEATURE_FIRMWARE").is_some() {
        embuild::espidf::sysenv::output();
    }
}
