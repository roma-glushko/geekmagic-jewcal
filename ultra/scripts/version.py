# PlatformIO pre-build script: JEWCAL_VERSION = <core version>-<git commit>,
# the same naming as the SmallTV Pro release (`make zip`).

import os
import re
import subprocess

Import("env")  # noqa: F821  (provided by PlatformIO)

root = env.subst("$PROJECT_DIR")  # noqa: F821
with open(os.path.join(root, "core", "Cargo.toml")) as f:
    version = re.search(r'^version\s*=\s*"([^"]+)"', f.read(), re.M).group(1)
try:
    commit = subprocess.check_output(
        ["git", "rev-parse", "--short", "HEAD"], cwd=root, stderr=subprocess.DEVNULL, text=True
    ).strip()
except Exception:
    commit = "dev"
env.Append(CPPDEFINES=[("JEWCAL_VERSION", env.StringifyMacro("%s-%s" % (version, commit)))])  # noqa: F821
