{
  description = "Canicula OS development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    { nixpkgs, rust-overlay, ... }:
    let
      supportedSystems = [
        "aarch64-darwin"
        "aarch64-linux"
        "x86_64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs supportedSystems;
    in
    {
      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          # The default image is x86_64, so use x86 OVMF even when the host is
          # Apple Silicon. Firmware is architecture data and can be consumed by
          # the native QEMU package without executing Linux binaries.
          x86LinuxPkgs = import nixpkgs { system = "x86_64-linux"; };
          x86Ovmf = x86LinuxPkgs.OVMF.fd;
        in
        rec {
          dev = pkgs.mkShell {
            packages =
              (with pkgs; [
                cargo-binutils
                cpio
                e2fsprogs
                gnumake
                gzip
                nixfmt
                qemu
                rust-analyzer
                rustToolchain
              ])
              ++ [ x86Ovmf ]
              ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isLinux [ pkgs.gdb ]
              ++ pkgs.lib.optionals pkgs.stdenv.hostPlatform.isDarwin [ pkgs.llvmPackages.lldb ];

            RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
            CANICULA_OVMF_CODE = "${x86Ovmf}/FV/OVMF_CODE.fd";
            CANICULA_OVMF_VARS_TEMPLATE = "${x86Ovmf}/FV/OVMF_VARS.fd";

            shellHook = ''
              echo "Canicula development environment"
              echo "  Rust: $(rustc --version)"
              echo "  Build: make"
            '';
          };

          default = dev;
        }
      );
    };
}
