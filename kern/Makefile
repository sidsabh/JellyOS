ROOT := $(shell git rev-parse --show-toplevel)

KERN := kernel
TARGET := target/aarch64-unknown-none/release/${KERN}
TTY_PATH := /dev/tty.SLAB_USBtoUART
SDCARD ?= $(ROOT)/user/fs.img

.PHONY: all build qemu transmit objdump nm check clean install test

all: build

build:
	@echo "+ Building build/$(KERN).elf [build/$@]"
	@cargo build --release $(if $(FEATURES),--features $(FEATURES))
	@mkdir -p build
	@cp -f $(TARGET) build/$(KERN).elf

	@echo "+ Building build/$(KERN).bin [objcopy]"
	# cargo objcopy --bin $(KERN) --release $(if $(FEATURES),--features $(FEATURES)) -- -O binary --strip-all
	/opt/homebrew/opt/llvm/bin/llvm-objcopy -O binary build/$(KERN).elf build/$(KERN).bin


build-all:
	@(cd ../ext/uspi/lib; make)
	cp -f ../ext/uspi/lib/libuspi.a ./.cargo/
	@(cd ../ext/uspi/env/lib; make)
	cp -f ../ext/uspi/env/lib/libuspienv.a ./.cargo/
	touch src/main.rs
	make build

check:
	@cargo check

qemu: build
	@# ./qemu.sh build/$(KERN).bin
	./qemu.sh build/$(KERN).bin -drive file=$(SDCARD),format=raw,if=sd $(QEMU_ARGS)

qemu-gdb: build
	./qemu.sh build/$(KERN).bin -drive file=$(SDCARD),format=raw,if=sd -S -s

qemu-asm: build
	./qemu.sh build/$(KERN).bin -drive file=$(SDCARD),format=raw,if=sd -d in_asm

transmit: FEATURES := transmit
transmit: build
	@echo "+ Transmitting build/$(KERN).bin to $(TTY_PATH)"
	ttywrite -i build/$(KERN).bin $(TTY_PATH)
	screen $(TTY_PATH) 115200
	reset


run: build
	./run.sh

objdump: build
	/opt/homebrew/opt/llvm/bin/llvm-objdump -d -S \
		--mcpu=cortex-a53 \
		--triple=aarch64-none-elf \
		build/$(KERN).elf | rustfilt | less -R


nm: build
	cargo nm build/$(KERN).elf

clean:
	cargo clean
	rm -rf build

test:
	cargo test --target=$(shell $(ROOT)/bin/get-host-target.sh)
