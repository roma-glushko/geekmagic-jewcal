# PlatformIO pre-build script: build the Rust core (calendar model + screens)
# for the ESP8266 and hand it to the Arduino linker.
#
# The core is a `staticlib` for `xtensa-esp8266-none-elf`. Two things stop it
# from linking as-is:
#
# * The Arduino linker script puts code in flash only for objects whose file
#   name ends in `.c.o`/`.cpp.o` (or for a fixed list of libraries). Anything
#   else lands in the 32 KB of IRAM and overflows it. The archive members are
#   therefore renamed to `*.c.o`.
# * `.rodata` goes to DRAM, where it would eat the heap (fonts, sprites, time
#   zones, strings...). It is renamed to `.irom0.pstr.*` (the section the core
#   uses for PROGMEM strings), so it stays in flash. Reading bytes from flash
#   needs the core's non-32-bit access handler: build with -DNON32XFER_HANDLER.
#
# compiler_builtins is left out. It would bring weak copies of memcpy and of
# the libgcc helpers (__udivsi3, __umodsi3, __muldi3, soft float...), and the
# linker would pick them over the ROM's. That hands the whole firmware
# flash-resident division, including the SDK's and the UART's interrupt
# handlers, which then crash whenever an interrupt divides while flash is
# being written (OTA, EEPROM, WiFi config). Without it, Rust's calls resolve
# to the ROM and libgcc like everyone else's.

import os
import shutil
import subprocess

Import("env")  # noqa: F821  (provided by PlatformIO)

PROJECT = env.subst("$PROJECT_DIR")  # noqa: F821
CORE = os.path.join(PROJECT, "core")
TARGET = "xtensa-esp8266-none-elf"
BUILD = os.path.join(env.subst("$BUILD_DIR"), "rust")  # noqa: F821
LIB = "jewcal_core"

TOOLS = os.path.join(env.PioPlatform().get_package_dir("toolchain-xtensa"), "bin")  # noqa: F821
AR = os.path.join(TOOLS, "xtensa-lx106-elf-ar")
OBJCOPY = os.path.join(TOOLS, "xtensa-lx106-elf-objcopy")
READELF = os.path.join(TOOLS, "xtensa-lx106-elf-readelf")


def cargo_build():
    features = env.GetProjectOption("custom_core_features", "")  # noqa: F821
    cmd = "cargo build --release"
    if features:
        cmd += " --features " + features
    export = os.path.expanduser("~/export-esp.sh")
    if os.path.exists(export):
        cmd = "source %s >/dev/null && %s" % (export, cmd)
    print("rust core: " + cmd)
    subprocess.check_call(["bash", "-c", cmd], cwd=CORE)
    return os.path.join(CORE, "target", TARGET, "release", "libjewcal_ultra_core.a")


def section_names(obj):
    out = subprocess.check_output([READELF, "-S", "-W", obj], text=True)
    names = []
    for line in out.splitlines():
        line = line.strip()
        if not line.startswith("[") or "]" not in line:
            continue
        rest = line.split("]", 1)[1].split()
        if rest:
            names.append(rest[0])
    return names


def relocate(archive):
    work = os.path.join(BUILD, "objs")
    shutil.rmtree(work, ignore_errors=True)
    os.makedirs(work)
    subprocess.check_call([AR, "x", archive], cwd=work)
    members = []
    for name in sorted(os.listdir(work)):
        src = os.path.join(work, name)
        if name.startswith("compiler_builtins"):
            os.remove(src)
            continue
        args = []
        for sec in section_names(src):
            if sec == ".rodata" or sec.startswith(".rodata."):
                args.append("--rename-section=%s=.irom0.pstr%s" % (sec, sec))
        dst = os.path.join(work, os.path.splitext(name)[0] + ".c.o")
        rsp = dst + ".rsp"
        with open(rsp, "w") as f:
            f.write("\n".join(args))
        subprocess.check_call([OBJCOPY, "@" + rsp, src, dst] if args else [OBJCOPY, src, dst])
        os.remove(src)
        os.remove(rsp)
        members.append(dst)
    out = os.path.join(BUILD, "lib%s.a" % LIB)
    if os.path.exists(out):
        os.remove(out)
    subprocess.check_call([AR, "rcs", out] + members)
    return out


os.makedirs(BUILD, exist_ok=True)
relocate(cargo_build())
env.Append(LIBPATH=[BUILD], LIBS=[LIB])  # noqa: F821
