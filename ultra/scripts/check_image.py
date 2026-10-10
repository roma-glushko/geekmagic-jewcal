# Checks on the built SmallTV Ultra images (`make ultra-check`, run by CI).
# Each guards a constraint that only shows up on the device, if at all:
#
# * the installer must fit through the stock firmware's updater (~536 KB free
#   with Ultra-V9.0.50), with margin for other stock versions;
# * the firmware must fit the 1 MB sketch area;
# * no libgcc/ROM helper (division, memcpy...) may live in flash: interrupt
#   handlers call them while flash is being written (OTA, EEPROM);
# * static RAM is what the heap does not get (~40 KB of heap with WiFi up);
# * the Rust core's constants must stay in flash, not in RAM.
#
#   python3 check_image.py BUILD_DIR TOOLCHAIN_BIN

import os
import subprocess
import sys

build, tools = sys.argv[1], sys.argv[2]
KIB = 1024
LIMITS = {
    "installer": 420 * KIB,
    "jewcal": 960 * KIB,
}
STATIC_RAM_MAX = 35 * KIB
DRAM_RODATA_MAX = 4 * KIB
ROM_HELPERS = ["__udivsi3", "__umodsi3", "__divsi3", "__udivdi3", "__umoddi3", "__divdi3", "__muldi3",
               "memcpy", "memset", "memmove", "memcmp"]
FLASH = 0x40200000

failed = []


def check(ok, what):
    print(("ok    " if ok else "FAIL  ") + what)
    if not ok:
        failed.append(what)


for env, limit in LIMITS.items():
    size = os.path.getsize(os.path.join(build, env, "firmware.bin"))
    check(size <= limit, f"{env}.bin is {size // KIB} KiB (limit {limit // KIB} KiB)")

elf = os.path.join(build, "jewcal", "firmware.elf")
nm = subprocess.check_output([os.path.join(tools, "xtensa-lx106-elf-nm"), elf], text=True)
where = {}
for line in nm.splitlines():
    parts = line.split()
    if len(parts) == 3 and parts[2] in ROM_HELPERS:
        where[parts[2]] = int(parts[0], 16)
in_flash = sorted(name for name, addr in where.items() if addr >= FLASH)
check(not in_flash, "ROM/libgcc helpers outside flash" + (f" (in flash: {', '.join(in_flash)})" if in_flash else ""))

sizes = {}
out = subprocess.check_output([os.path.join(tools, "xtensa-lx106-elf-size"), "-A", elf], text=True)
for line in out.splitlines():
    parts = line.split()
    if len(parts) >= 2 and parts[0] in (".data", ".rodata", ".bss") and parts[1].isdigit():
        sizes[parts[0]] = int(parts[1])
static = sum(sizes.values())
check(static <= STATIC_RAM_MAX, f"static RAM {static} B (limit {STATIC_RAM_MAX} B; the heap gets the rest of 80 KB)")
check(sizes.get(".rodata", 0) <= DRAM_RODATA_MAX,
      f".rodata in RAM {sizes.get('.rodata', 0)} B (limit {DRAM_RODATA_MAX} B: Rust constants must be in flash)")

if failed:
    sys.exit(1)
