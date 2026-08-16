fn main() {
    if std::env::var_os("CARGO_FEATURE_NAPI_HOST").is_some() {
        napi_build::setup();
    }
}
