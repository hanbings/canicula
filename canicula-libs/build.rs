use std::fs;
use toml::Value;

fn main() {
    let common_config = "../build-config/common.toml";

    #[cfg(target_arch = "x86_64")]
    let libs_config = "../build-config/x86_64/x86_64-libs.toml";
    #[cfg(target_arch = "aarch64")]
    let libs_config = "../build-config/aarch64/aarch64-libs.toml";
    #[cfg(target_arch = "riscv64")]
    let libs_config = "../build-config/riscv64/riscv64-libs.toml";

    let common_content = fs::read_to_string(common_config).unwrap();
    let common_value: Value = toml::from_str(&common_content).unwrap();

    let libs_content = fs::read_to_string(libs_config).unwrap();
    let libs_value: Value = toml::from_str(&libs_content).unwrap();

    for (key, value) in common_value.as_table().unwrap() {
        println!("cargo::rustc-env={}={}", key, value);
    }

    for (key, value) in libs_value.as_table().unwrap() {
        println!("cargo::rustc-env={}={}", key, value);
    }
}
