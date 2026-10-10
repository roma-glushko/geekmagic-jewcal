# JewCal: build, test, flash and package.
#
# SmallTV Pro (ESP32, firmware/):
#   make              build the firmware and zip it (same as `make zip`)
#   make zip          firmware -> dist/jewcal-pro-<version>-<commit>.zip
#   make build        host crates + ESP32 firmware
#   make firmware     ESP32 firmware only
#   make test         host tests (calendar model + screens, both canvas layouts)
#   make check        what CI checks on the host: formatting, lints, tests, no_std builds
#   make firmware-check  clippy on the ESP32 firmware
#   make gallery      render every screen to shots/index.html
#   make flash        build, flash over USB, open the serial monitor
#                     (the very first time, backs up the device's firmware)
#   make flash-full   flash the release image at 0x0 (fresh install, clears settings)
#   make flash-app    flash only the app at 0x10000 (update, keeps settings)
#   make backup       save the whole 4 MB flash to firmware/backup/
#   make restore      write a backup back: make restore BACKUP=firmware/backup/<file>.bin
#   make monitor      serial monitor only
#   make clean        remove build outputs (both devices), dist/ and shots/
#
# SmallTV Ultra (ESP8266, ultra/):
#   make ultra        installer + firmware images (PlatformIO + the Rust core)
#   make ultra-zip    -> dist/jewcal-ultra-<version>-<commit>.zip
#   make ultra-ota    upload the firmware to a running JewCal: make ultra-ota HOST=jewcal.local
#   make ultra-stack  deepest stack use per entry point (the loop stack is 4 KB)
#   make ultra-check  CI's checks on the images: sizes, ROM helpers, RAM, stack budget
#
# Options:
#   FEATURES=backlight-gpio21   board variant (comma-separated cargo features)
#   PORT=/dev/cu.usbserial-110  serial port for flash/monitor (auto-detected otherwise)
#   HOST=192.168.1.23           the SmallTV Ultra for ultra-ota (default jewcal.local)

SHELL := /bin/bash
.DEFAULT_GOAL := zip

comma := ,
# The esp toolchain's environment (written by espup). CI exports it itself.
ESP_ENV := { [ ! -f $(HOME)/export-esp.sh ] || . $(HOME)/export-esp.sh; }
FW_DIR  := firmware
ELF     := $(FW_DIR)/target/xtensa-esp32-espidf/release/jewcal
VERSION := $(shell sed -n 's/^version *= *"\(.*\)"/\1/p' $(FW_DIR)/Cargo.toml | head -1)
COMMIT  := $(shell git rev-parse --short HEAD 2>/dev/null || echo dev)
NAME    := jewcal-pro-$(VERSION)-$(COMMIT)$(if $(FEATURES),-$(subst $(comma),-,$(FEATURES)))
DIST    := dist
PKG     := $(DIST)/$(NAME)

FEATURE_ARGS := $(if $(FEATURES),--features $(FEATURES))
PORT_ARGS    := $(if $(PORT),--port $(PORT))
# The SmallTV Pro has 4 MB of flash; images are built for that layout.
IMAGE_ARGS   := --chip esp32 --flash-size 4mb
# Every write names the chip, so espflash refuses an ESP8266 (SmallTV Ultra).
CHIP_ARGS    := --chip esp32

BACKUP_DIR   := $(FW_DIR)/backup
FLASH_BYTES  := 0x400000

# SmallTV Ultra
PIO          := $(shell command -v pio 2>/dev/null || echo $(HOME)/.platformio/penv/bin/pio)
ULTRA_DIR    := ultra
ULTRA_BUILD  := $(ULTRA_DIR)/.pio/build
ULTRA_VERSION := $(shell sed -n 's/^version *= *"\(.*\)"/\1/p' $(ULTRA_DIR)/core/Cargo.toml | head -1)
ULTRA_NAME   := jewcal-ultra-$(ULTRA_VERSION)-$(COMMIT)
ULTRA_PKG    := $(DIST)/$(ULTRA_NAME)
HOST         ?= jewcal.local

.PHONY: zip build firmware test check firmware-check gallery flash flash-full flash-app backup restore monitor \
	clean help ultra ultra-zip ultra-ota ultra-stack ultra-check

help:
	@sed -n '2,49p' Makefile | sed 's/^# \{0,1\}//'

test:
	cargo test
	cargo test -q -p jewcal-ui --features packed

# Everything CI checks on the host. The firmware crates are formatted with the
# host's rustfmt (they are outside the workspace).
check:
	cargo fmt --all --check
	rustfmt --edition 2021 --check $(FW_DIR)/src/*.rs $(ULTRA_DIR)/core/src/*.rs
	cargo clippy --all-targets -- -D warnings
	$(MAKE) --no-print-directory test
	cargo build -q -p jewcal-model --no-default-features
	cargo build -q -p jewcal-ui --no-default-features --features packed
	cargo build -q -p jewcal-app --no-default-features

firmware-check:
	$(ESP_ENV) && cd $(FW_DIR) && cargo clippy --release $(FEATURE_ARGS) -- -D warnings

gallery:
	cargo run -q -p jewcal-sim -- gallery --out shots

build: firmware
	cargo build

firmware:
	$(ESP_ENV) && cd $(FW_DIR) && cargo build --release $(FEATURE_ARGS)

# Merged image (bootloader + partition table + app, written at 0x0), the app
# alone (0x10000, keeps settings on update), the ELF for decoding backtraces,
# checksums and flashing instructions, zipped.
zip: firmware
	@mkdir -p $(PKG)
	$(ESP_ENV) && espflash save-image $(IMAGE_ARGS) --merge --skip-padding $(ELF) $(PKG)/jewcal-pro-full.bin
	$(ESP_ENV) && espflash save-image $(IMAGE_ARGS) $(ELF) $(PKG)/jewcal-pro-app.bin
	cp $(ELF) $(PKG)/jewcal-pro.elf
	sed -e 's|@NAME@|$(NAME)|g' $(FW_DIR)/FLASHING.txt.in > $(PKG)/FLASHING.txt
	install -m 0755 $(FW_DIR)/flash-package.sh $(PKG)/flash.sh
	cd $(PKG) && shasum -a 256 jewcal-pro-full.bin jewcal-pro-app.bin jewcal-pro.elf > SHA256SUMS
	cd $(DIST) && zip -q -FS -r $(NAME).zip $(NAME)
	@echo "Packaged $(DIST)/$(NAME).zip"
	@unzip -l $(DIST)/$(NAME).zip | tail -n +4 | sed '$$d' | sed '$$d'

# The first flash on this machine saves what the device was running (the
# stock firmware), so it can always be restored with `make restore`.
flash: firmware
	@if ! ls $(BACKUP_DIR)/smalltv-*.bin >/dev/null 2>&1; then \
		echo "First flash: backing up the current firmware to $(BACKUP_DIR)/ (once)"; \
		$(MAKE) --no-print-directory backup; \
	fi
	$(ESP_ENV) && espflash flash $(CHIP_ARGS) $(PORT_ARGS) --monitor $(ELF)

flash-full: zip
	$(ESP_ENV) && espflash write-bin $(CHIP_ARGS) $(PORT_ARGS) 0x0 $(PKG)/jewcal-pro-full.bin

flash-app: zip
	$(ESP_ENV) && espflash write-bin $(CHIP_ARGS) $(PORT_ARGS) 0x10000 $(PKG)/jewcal-pro-app.bin

backup:
	@mkdir -p $(BACKUP_DIR)
	$(ESP_ENV) && espflash read-flash $(PORT_ARGS) 0 $(FLASH_BYTES) $(BACKUP_DIR)/smalltv-$$(date +%Y%m%d-%H%M%S).bin
	@ls -la $(BACKUP_DIR)

restore:
	@test -n "$(BACKUP)" || { echo "usage: make restore BACKUP=$(BACKUP_DIR)/<file>.bin"; exit 1; }
	@test -f "$(BACKUP)" || { echo "no such file: $(BACKUP)"; exit 1; }
	$(ESP_ENV) && espflash write-bin $(CHIP_ARGS) $(PORT_ARGS) 0x0 $(BACKUP)

monitor:
	$(ESP_ENV) && espflash monitor $(PORT_ARGS) --elf $(ELF)

clean:
	cargo clean
	$(ESP_ENV) && cd $(FW_DIR) && cargo clean
	$(ESP_ENV) && cd $(ULTRA_DIR)/core && cargo clean
	rm -rf $(DIST) shots $(ULTRA_DIR)/.pio

# ── SmallTV Ultra (ESP8266) ─────────────────────────────────────────────────

# Both images: the installer (C++ only, fits the stock updater) and the
# firmware (C++ + the Rust core, built by ultra/scripts/rust_core.py).
ultra:
	@test -x "$(PIO)" || { echo "PlatformIO not found: pip install platformio (or see README)"; exit 1; }
	cd $(ULTRA_DIR) && "$(PIO)" run

ultra-zip: ultra
	@mkdir -p $(ULTRA_PKG)
	cp $(ULTRA_BUILD)/installer/firmware.bin $(ULTRA_PKG)/jewcal-ultra-installer.bin
	cp $(ULTRA_BUILD)/jewcal/firmware.bin $(ULTRA_PKG)/jewcal-ultra.bin
	cp $(ULTRA_BUILD)/jewcal/firmware.elf $(ULTRA_PKG)/jewcal-ultra.elf
	sed -e 's|@NAME@|$(ULTRA_NAME)|g' $(ULTRA_DIR)/INSTALL.txt.in > $(ULTRA_PKG)/INSTALL.txt
	cd $(ULTRA_PKG) && shasum -a 256 jewcal-ultra-installer.bin jewcal-ultra.bin jewcal-ultra.elf > SHA256SUMS
	cd $(DIST) && zip -q -FS -r $(ULTRA_NAME).zip $(ULTRA_NAME)
	@echo "Packaged $(DIST)/$(ULTRA_NAME).zip"
	@unzip -l $(DIST)/$(ULTRA_NAME).zip | tail -n +4 | sed '$$d' | sed '$$d'

# Update a SmallTV Ultra that already runs JewCal. Its update page has a
# per-boot address, linked from the settings page (and from safe mode).
ultra-ota: ultra
	@path=$$(curl -fsS http://$(HOST)/ | grep -o '/update-[0-9a-f]*' | head -1); \
	test -n "$$path" || { echo "No update link at http://$(HOST)/ (is it running JewCal?)"; exit 1; }; \
	echo "Uploading to http://$(HOST)$$path"; \
	curl -fS --progress-bar -F "firmware=@$(ULTRA_BUILD)/jewcal/firmware.bin" "http://$(HOST)$$path"
	@echo; echo "Uploaded; JewCal restarts in a few seconds."

# Deepest stack use from the Arduino loop and each Rust entry point (4 KB budget).
STACK_ROOTS := loop jc_frame jc_feed jc_finish jc_next_request jc_settings_page jc_form_apply
XTENSA_BIN  = $(HOME)/.platformio/packages/toolchain-xtensa/bin
define ultra_dump
	$(XTENSA_BIN)/xtensa-lx106-elf-objdump -d -C --no-show-raw-insn $(ULTRA_BUILD)/jewcal/firmware.elf > $(ULTRA_BUILD)/jewcal/code.txt
	$(XTENSA_BIN)/xtensa-lx106-elf-objdump -s -j .irom0.text -j .text -j .text1 $(ULTRA_BUILD)/jewcal/firmware.elf > $(ULTRA_BUILD)/jewcal/literals.txt
endef

ultra-stack: ultra
	@$(ultra_dump)
	@python3 $(ULTRA_DIR)/scripts/stack_depth.py $(ULTRA_BUILD)/jewcal/code.txt $(ULTRA_BUILD)/jewcal/literals.txt \
	  $(STACK_ROOTS) | grep -E 'worst|^ +[0-9]' | awk '/worst/ {print; n=0; next} {if (++n <= 4) print}'

# The Ultra's hard limits, checked on the built images (CI runs this). The
# 3 KB stack budget leaves ~1 KB of the 4 KB for interrupt frames and the
# lwIP calls behind the page sink, which the static analysis cannot see.
ultra-check: ultra
	python3 $(ULTRA_DIR)/scripts/check_image.py $(ULTRA_BUILD) $(XTENSA_BIN)
	@$(ultra_dump)
	@python3 $(ULTRA_DIR)/scripts/stack_depth.py --max 3072 $(ULTRA_BUILD)/jewcal/code.txt \
	  $(ULTRA_BUILD)/jewcal/literals.txt $(STACK_ROOTS) | grep -E 'worst|ok:|FAIL'
