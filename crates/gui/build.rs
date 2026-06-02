fn main() {
    #[cfg(target_os = "windows")]
    println!("cargo:rustc-link-arg=/SUBSYSTEM:WINDOWS");
    #[cfg(target_os = "linux")]
    println!("cargo:rustc-link-arg=-Wl,-subsystem,windows");
}
