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
	cargo build -p canicula-loader \
		-Zbuild-std=core,alloc \
		-Zbuild-std-features=compiler-builtins-mem \
		--release \
		--target x86_64-unknown-uefi
	mkdir -p esp/efi/boot/
	cp target/x86_64-unknown-uefi/release/canicula-loader.efi esp/efi/boot/bootx64.efi

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

initramfs:
	@echo "Building minimal initramfs with busybox..."
	rm -rf initramfs
	mkdir -p initramfs/{bin,sbin,etc,proc,sys,dev,tmp,lib/x86_64-linux-gnu,lib64,usr/bin,usr/sbin,var/run,root,mnt}
	cp /bin/busybox initramfs/bin/
	cp /lib/x86_64-linux-gnu/libresolv.so.2 initramfs/lib/x86_64-linux-gnu/
	cp /lib/x86_64-linux-gnu/libc.so.6 initramfs/lib/x86_64-linux-gnu/
	cp /lib64/ld-linux-x86-64.so.2 initramfs/lib64/
	for applet in $$(busybox --list); do ln -sf busybox initramfs/bin/$$applet 2>/dev/null; done
	for cmd in init mount umount poweroff reboot halt switch_root; do ln -sf ../bin/busybox initramfs/sbin/$$cmd; done
	printf '#!/bin/sh\nmount -t proc none /proc\nmount -t sysfs none /sys\nmount -t devtmpfs none /dev\nmkdir -p /dev/pts\nmount -t devpts none /dev/pts\nhostname canicula\necho ""\necho "  Canicula Linux Boot - Initramfs"\necho "  Kernel: $$(uname -r)"\necho ""\nexec /bin/sh\n' > initramfs/init
	chmod +x initramfs/init
	echo "root:x:0:0:root:/root:/bin/sh" > initramfs/etc/passwd
	echo "root:x:0:" > initramfs/etc/group
	(cd initramfs && find . | cpio -H newc -o --quiet | gzip -9) > initrd.img
	@echo "initrd.img created ($$(du -h initrd.img | cut -f1))"

vmlinuz: efi initramfs
	mkdir -p esp/efi/boot/
	cp target/x86_64-unknown-uefi/release/canicula-loader.efi esp/efi/boot/bootx64.efi
	@if [ -f vmlinuz-* ]; then \
		cp vmlinuz-* esp/vmlinuz; \
		echo "Copied vmlinuz to esp/vmlinuz"; \
	else \
		echo "Warning: No vmlinuz file found in project root"; \
	fi
	cp initrd.img esp/initrd.img
	@echo "Copied initrd.img to esp/initrd.img"

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

.PHONY: efi kernel initramfs vmlinuz clean qemu kill-qemu clean-esp all