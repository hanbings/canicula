use std::fs;
use toml::Value;

fn main() {
    let common_config = "../build-config/common.toml";

    #[cfg(target_arch = "x86_64")]
    let kernel_config = "../build-config/x86_64/x86_64-kernel.toml";
    #[cfg(target_arch = "aarch64")]
    let kernel_config = "../build-config/aarch64/aarch64-kernel.toml";
    #[cfg(target_arch = "riscv64")]
    let kernel_config = "../build-config/riscv64/riscv64-kernel.toml";

    let common_content = fs::read_to_string(common_config).unwrap();
    let common_value: Value = toml::from_str(&common_content).unwrap();

    let kernel_content = fs::read_to_string(kernel_config).unwrap();
    let kernel_value: Value = toml::from_str(&kernel_content).unwrap();

    for (key, value) in common_value.as_table().unwrap() {
        println!("cargo::rustc-env={}={}", key, value);
    }

    for (key, value) in kernel_value.as_table().unwrap() {
        println!("cargo::rustc-env={}={}", key, value);
    }
}
