use std::fs;
use toml::Value;

fn main() {
    let common_config = "../build-config/common.toml";

    #[cfg(target_arch = "x86_64")]
    let efi_config = "../build-config/x86_64/x86_64-efi.toml";
    #[cfg(target_arch = "aarch64")]
    let efi_config = "../build-config/aarch64/aarch64-efi.toml";

    let common_content = fs::read_to_string(common_config).unwrap();
    let common_value: Value = toml::from_str(&common_content).unwrap();

    let efi_content = fs::read_to_string(efi_config).unwrap();
    let efi_value: Value = toml::from_str(&efi_content).unwrap();

    for (key, value) in common_value.as_table().unwrap() {
        println!("cargo::rustc-env={}={}", key, value);
    }

    for (key, value) in efi_value.as_table().unwrap() {
        println!("cargo::rustc-env={}={}", key, value);
    }
}