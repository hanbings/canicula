use std::fs;
use toml::Value;

fn main() {
    let common_config = "../build-config/common.toml";

    #[cfg(target_arch = "x86_64")]
    let fs_config = "../build-config/x86_64/x86_64-fs.toml";
    #[cfg(target_arch = "aarch64")]
    let fs_config = "../build-config/aarch64/aarch64-fs.toml";
    #[cfg(target_arch = "riscv64")]
    let fs_config = "../build-config/riscv64/riscv64-fs.toml";

    let common_content = fs::read_to_string(common_config).unwrap();
    let common_value: Value = toml::from_str(&common_content).unwrap();

    let fs_content = fs::read_to_string(fs_config).unwrap();
    let fs_value: Value = toml::from_str(&fs_content).unwrap();

    for (key, value) in common_value.as_table().unwrap() {
        println!("cargo::rustc-env={}={}", key, value);
    }

    for (key, value) in fs_value.as_table().unwrap() {
        println!("cargo::rustc-env={}={}", key, value);
    }
}
