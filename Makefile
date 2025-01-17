OS := $(shell uname)
DISTRO := $(shell cat /etc/*release | grep '^ID=' | cut -d '=' -f2)

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
	cargo build --bin canicula-efi --target x86_64-unknown-uefi
	mkdir -p esp/efi/boot/
	cp target/x86_64-unknown-uefi/debug/canicula-efi.efi esp/efi/boot/bootx64.efi

kernel:
	cargo build --bin canicula-kernel --target canicula-kernel/x86_64-unknown-none.json
	mkdir -p esp
	cp target/x86_64-unknown-none/debug/canicula-kernel esp/canicula-kernel

clean:
	rm -rf target
	rm -rf esp

clean-esp:
	rm -rf esp

qemu:
	qemu-system-x86_64 \
		-m 256 \
	    -enable-kvm \
		-nographic \
		-s -S \
        -drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE_PATH) \
        -drive if=pflash,format=raw,readonly=on,file=$(OVMF_VARS_PATH) \
        -drive format=raw,file=fat:rw:esp

kill-qemu:
	pgrep qemu | xargs kill -9

.PHONY: efi kernel clean qemu kill-qemu clean-esp all