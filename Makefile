OS := $(shell uname)
DISTRO := $(shell cat /etc/*release | grep '^ID=' | cut -d '=' -f2)
LOG_LEVEL ?= DEBUG

OVMF_CODE_PATH := /usr/share/OVMF/OVMF_CODE.fd
OVMF_VARS_PATH := /usr/share/OVMF/OVMF_VARS.fd

ifeq ($(OS),Linux)
    ifeq ($(DISTRO),arch)
		OVMF_CODE_PATH := /usr/share/OVMF/x64/OVMF_CODE.4m.fd
	    OVMF_VARS_PATH := /usr/share/OVMF/x64/OVMF_VARS.4m.fd
	endif

	ifeq ($(DISTRO),debian)
		OVMF_CODE_PATH := /usr/share/OVMF/OVMF_CODE_4M.fd
	    OVMF_VARS_PATH := /usr/share/OVMF/OVMF_VARS_4M.fd
	endif
endif

$(info OS=$(OS))
$(info DISTRO=$(DISTRO))
$(info OVMF_CODE_PATH=$(OVMF_CODE_PATH))
$(info OVMF_VARS_PATH=$(OVMF_VARS_PATH))

all: efi kernel

efi:
	cd bootloader/uefi && cargo build \
		-Zbuild-std=core \
		-Zbuild-std-features=compiler-builtins-mem \
		--release \
		--target x86_64-unknown-uefi
	mkdir -p esp/efi/boot/
	cp bootloader/target/x86_64-unknown-uefi/release/bootloader-x86_64-uefi.efi esp/efi/boot/bootx64.efi

kernel:
	LOG_LEVEL=$(LOG_LEVEL) cargo build \
	    -Z build-std=core,alloc,compiler_builtins \
	    -Z build-std-features=compiler-builtins-mem \
		-Z json-target-spec -p canicula-kernel \
		--release \
		--target canicula-kernel/x86_64-canicula-kernel.json
	mkdir -p esp
	cp target/x86_64-canicula-kernel/release/canicula-kernel esp/kernel-x86_64

clean:
	rm -rf target
	rm -rf esp

clean-esp:
	rm -rf esp

qemu:
	qemu-system-x86_64 \
    -m 256M \
    -serial stdio \
    -enable-kvm \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE_PATH) \
    -drive if=pflash,format=raw,readonly=on,file=$(OVMF_VARS_PATH) \
    -drive format=raw,file=fat:rw:esp

kill-qemu:
	pgrep qemu | xargs kill -9

.PHONY: efi kernel clean qemu kill-qemu clean-esp all