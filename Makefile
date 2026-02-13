OS := $(shell uname)
DISTRO := $(shell cat /etc/*release | grep '^ID=' | cut -d '=' -f2)
LOG_LEVEL ?= DEBUG
KERNEL_FEATURES ?=
SMP ?= 4

KERNEL_VERSION ?= $(shell uname -r)
VMLINUZ_SRC ?= /boot/vmlinuz-$(KERNEL_VERSION)
BUSYBOX ?= $(shell command -v busybox 2>/dev/null)

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
		$(if $(KERNEL_FEATURES),--features $(KERNEL_FEATURES),) \
		--target canicula-kernel/x86_64-canicula-kernel.json
	mkdir -p esp
	cp target/x86_64-canicula-kernel/release/canicula-kernel esp/kernel-x86_64

clean:
	rm -rf target
	rm -rf esp

initramfs:
	@echo "Building minimal initramfs with busybox..."
	@if [ -z "$(BUSYBOX)" ] || [ ! -f "$(BUSYBOX)" ]; then \
		echo "Error: busybox not found."; \
		echo "  Install busybox (e.g. apt install busybox-static) or set BUSYBOX=/path/to/busybox"; \
		exit 1; \
	fi
	rm -rf initramfs
	mkdir -p initramfs/bin initramfs/sbin initramfs/etc initramfs/proc initramfs/sys \
		initramfs/dev initramfs/tmp initramfs/usr/bin initramfs/usr/sbin \
		initramfs/var/run initramfs/root initramfs/mnt
	cp $(BUSYBOX) initramfs/bin/busybox
	@# Copy dynamic libraries if busybox is dynamically linked
	@if ldd "$(BUSYBOX)" >/dev/null 2>&1 && ! ldd "$(BUSYBOX)" 2>&1 | grep -q "not a dynamic"; then \
		echo "Busybox is dynamically linked, copying shared libraries..."; \
		ldd "$(BUSYBOX)" | grep -o '/[^ ]*' | while read lib; do \
			dir=$$(dirname "$$lib"); \
			mkdir -p "initramfs$$dir"; \
			cp "$$lib" "initramfs$$lib"; \
		done; \
	else \
		echo "Busybox is statically linked, no shared libraries needed."; \
	fi
	for applet in $$("$(BUSYBOX)" --list); do ln -sf busybox initramfs/bin/$$applet 2>/dev/null; done
	for cmd in init mount umount poweroff reboot halt switch_root; do ln -sf ../bin/busybox initramfs/sbin/$$cmd; done
	printf '#!/bin/sh\nmount -t proc none /proc\nmount -t sysfs none /sys\nmount -t devtmpfs none /dev\nmkdir -p /dev/pts\nmount -t devpts none /dev/pts\nhostname canicula\necho ""\necho "  Canicula Linux Boot - Initramfs"\necho "  Kernel: $$(uname -r)"\necho ""\nexec /bin/sh\n' > initramfs/init
	chmod +x initramfs/init
	echo "root:x:0:0:root:/root:/bin/sh" > initramfs/etc/passwd
	echo "root:x:0:" > initramfs/etc/group
	(cd initramfs && find . | cpio -H newc -o --quiet | gzip -9) > initrd.img
	@echo "initrd.img created ($$(du -h initrd.img | cut -f1))"

vmlinuz: efi initramfs
	@if [ ! -f "$(VMLINUZ_SRC)" ]; then \
		echo "Error: vmlinuz not found at $(VMLINUZ_SRC)"; \
		echo "  Set VMLINUZ_SRC=/path/to/vmlinuz or KERNEL_VERSION=x.y.z"; \
		echo "  Available kernels in /boot/:"; \
		ls /boot/vmlinuz-* 2>/dev/null || echo "    (none found)"; \
		exit 1; \
	fi
	mkdir -p esp/efi/boot/
	cp target/x86_64-unknown-uefi/release/canicula-loader.efi esp/efi/boot/bootx64.efi
	cp "$(VMLINUZ_SRC)" esp/vmlinuz
	@echo "Copied $(VMLINUZ_SRC) to esp/vmlinuz"
	cp initrd.img esp/initrd.img
	@echo "Copied initrd.img to esp/initrd.img"

clean-esp:
	rm -rf esp

qemu:
	qemu-system-x86_64 \
    -m 256M \
	-smp $(SMP) \
    -serial mon:stdio \
    -enable-kvm \
	-cpu host \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE_PATH) \
    -drive if=pflash,format=raw,readonly=on,file=$(OVMF_VARS_PATH) \
    -drive format=raw,file=fat:rw:esp \
	-nographic

qemu-debug:
	@echo "QEMU gdbstub listening on tcp:127.0.0.1:1234"
	@echo "Attach with: gdb -ex 'target remote :1234'"
	qemu-system-x86_64 \
    -m 256M \
	-smp $(SMP) \
    -serial mon:stdio \
	-enable-kvm \
    -s \
    -S \
    -no-reboot \
    -no-shutdown \
    -d int,cpu_reset \
    -D qemu-int.log \
	-cpu host \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    -drive if=pflash,format=raw,readonly=on,file=$(OVMF_CODE_PATH) \
    -drive if=pflash,format=raw,readonly=on,file=$(OVMF_VARS_PATH) \
    -drive format=raw,file=fat:rw:esp \
	-nographic

lldb-qemu:
	@echo "Connecting LLDB to QEMU gdbstub at 127.0.0.1:1234"
	lldb \
	  -o "target create /home/hanbings/github/canicula/target/x86_64-canicula-kernel/release/canicula-kernel" \
	  -o "gdb-remote 127.0.0.1:1234"

kill-qemu:
	pgrep qemu | xargs kill -9

.PHONY: efi kernel initramfs vmlinuz clean qemu qemu-debug lldb-qemu qemu-monitor qemu-debug-tcp-monitor kill-qemu clean-esp all