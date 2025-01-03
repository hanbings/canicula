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
	qemu-system-x86_64 -enable-kvm -nographic \
       -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_CODE.fd \
       -drive if=pflash,format=raw,readonly=on,file=/usr/share/OVMF/OVMF_VARS.fd \
       -drive format=raw,file=fat:rw:esp

kill-qemu:
	pgrep qemu | xargs kill -9

.PHONY: efi kernel clean qemu kill-qemu clean-esp all