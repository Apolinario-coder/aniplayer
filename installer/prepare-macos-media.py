#!/usr/bin/env python3
"""Copy Homebrew media tools and their dependencies into a private runtime."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile

root = Path(sys.argv[1]).resolve()
for name in ("bin", "lib", "rife/models/rife-v4.16lite", "licenses"):
    (root / name).mkdir(parents=True, exist_ok=True)

def output(*args):
    return subprocess.check_output(args, text=True).strip()

python = Path(output("brew", "--prefix", "vapoursynth")) / "libexec/bin/python"
if not python.is_file():
    raise RuntimeError("VapourSynth's matching Python interpreter is missing")
version = output(str(python), "-c", 'import sys;print(f"{sys.version_info.major}.{sys.version_info.minor}")')
stdlib = Path(output(str(python), "-c", 'import sysconfig;print(sysconfig.get_path("stdlib"))'))
shutil.copytree(stdlib, root / "lib" / f"python{version}", symlinks=False,
                ignore=shutil.ignore_patterns("site-packages", "__pycache__", "test", "tests"))
site = root / "lib" / f"python{version}" / "site-packages"
site.mkdir()
vs = Path(output(str(python), "-c", "import vapoursynth;print(vapoursynth.__file__)")).parent
shutil.copytree(vs, site / "vapoursynth", symlinks=False,
                ignore_dangling_symlinks=True, ignore=shutil.ignore_patterns("__pycache__"))
with tempfile.TemporaryDirectory() as wheels:
    subprocess.run([sys.executable, "-m", "pip", "download", "--no-deps", "--dest", wheels,
                    "yt-dlp==2026.10.21"], check=True)
    with zipfile.ZipFile(next(Path(wheels).glob("*.whl"))) as wheel:
        wheel.extractall(site)
arch = output("uname", "-m")
raw = "https://raw.githubusercontent.com/styler00dollar/VapourSynth-RIFE-ncnn-Vulkan/c3ec6aabc07c8fa37a4f58d7fed9e2ad1fc1b13f"
plugin = "librife_macos_arm64.dylib" if arch == "arm64" else "librife_macos_x86-64.dylib"
urllib.request.urlretrieve(
    "https://github.com/styler00dollar/VapourSynth-RIFE-ncnn-Vulkan/releases/download/r9_mod_v33/" + plugin,
    root / "rife/librife.dylib")
for name in ("flownet.bin", "flownet.param"):
    urllib.request.urlretrieve(raw + "/models/rife-v4.16_lite_ensembleFalse/" + name,
                               root / "rife/models/rife-v4.16lite" / name)
urllib.request.urlretrieve(raw + "/LICENSE", root / "licenses/RIFE.txt")

copied = {}
pending = []
def copy_binary(source, target):
    source = source.resolve()
    if source in copied:
        return copied[source]
    if source == target.resolve():
        return target
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, target)
    target.chmod(0o755)
    copied[source] = target
    copied[target.resolve()] = target
    pending.append((source, target))
    return target

for name in ("mpv", "ffmpeg", "python3"):
    executable = python if name == "python3" else Path(shutil.which(name))
    if name == "python3":
        framework_interpreter = stdlib.parent.parent / "Resources/Python.app/Contents/MacOS/Python"
        if framework_interpreter.is_file():
            executable = framework_interpreter
    copy_binary(executable, root / "bin" / (name + ".real"))
for library in vs.glob("*.dylib"):
    copy_binary(library, root / "lib" / library.name)
for file in list(root.rglob("*.so")) + list(root.rglob("*.dylib")):
    pending.append((file, file))
molten = Path(output("brew", "--prefix", "molten-vk"))
copy_binary(molten / "lib/libMoltenVK.dylib", root / "lib/libMoltenVK.dylib")
certificates = Path(output(str(python), "-c", "import ssl;print(ssl.get_default_verify_paths().cafile or '')"))
if not certificates.is_file():
    raise RuntimeError("The build environment has no CA certificate bundle")
shutil.copy2(certificates, root / "lib/cert.pem")

processed = set()
while pending:
    source, target = pending.pop()
    if target in processed:
        continue
    processed.add(target)
    changes = []
    dependencies = output("otool", "-L", str(source)).splitlines()[1:]
    for row in dependencies:
        dependency = row.strip().split(" (compatibility")[0]
        if dependency.startswith(("/usr/lib/", "/System/Library/")):
            continue
        if dependency.startswith("@loader_path/"):
            resolved = source.parent / dependency.removeprefix("@loader_path/")
        elif dependency.startswith("@executable_path/"):
            resolved = source.parent / dependency.removeprefix("@executable_path/")
        elif dependency.startswith("@rpath/"):
            tail = dependency.removeprefix("@rpath/")
            candidates = [source.parent / tail, source.parent.parent / "lib" / tail,
                          Path(output("brew", "--prefix")) / "lib" / tail]
            resolved = next((path for path in candidates if path.exists()), None)
            if resolved is None:
                raise RuntimeError(f"Unresolved dependency {dependency} in {source}")
        else:
            resolved = Path(dependency)
        if resolved.resolve() == source.resolve():
            continue
        relocated = copy_binary(resolved, root / "lib" / resolved.name)
        relative = os.path.relpath(relocated, target.parent)
        changes.extend(["-change", dependency, "@loader_path/" + relative])
    if target.suffix in (".so", ".dylib") or target.name == "Python":
        changes.extend(["-id", "@loader_path/" + target.name])
    if changes:
        subprocess.run(["install_name_tool", *changes, str(target)], check=True)

(root / "lib/MoltenVK_icd.json").write_text(json.dumps({
    "file_format_version": "1.0.0",
    "ICD": {"library_path": "libMoltenVK.dylib", "api_version": "1.3.0", "is_portability_driver": True}
}))
script = f'''#!/bin/sh
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd -P)
export DYLD_LIBRARY_PATH="$ROOT/lib"
export PYTHONHOME="$ROOT"
export PYTHONPATH="$ROOT/lib/python{version}/site-packages"
export SSL_CERT_FILE="$ROOT/lib/cert.pem"
export VSSCRIPT_PATH="$ROOT/lib/python{version}/site-packages/vapoursynth/libvsscript.dylib"
export XDG_CONFIG_HOME="${{ANI_PLAYER_CONFIG:-$HOME/Library/Application Support/Ani Player}}/media-config"
export PATH="$ROOT/bin:$PATH"
export VK_ICD_FILENAMES="$ROOT/lib/MoltenVK_icd.json"
exec "$ROOT/bin/$(basename "$0").real" "$@"
'''
for name in ("mpv", "ffmpeg", "python3"):
    path = root / "bin" / name
    path.write_text(script)
    path.chmod(0o755)
(root / "bin/yt-dlp").write_text('#!/bin/sh\nexec "$(dirname "$0")/python3" -m yt_dlp "$@"\n')
(root / "bin/yt-dlp").chmod(0o755)
(root / "bin/vapoursynth").write_text('#!/bin/sh\nexec "$(dirname "$0")/python3" -m vapoursynth "$@"\n')
(root / "bin/vapoursynth").chmod(0o755)
for file in root.rglob("*"):
    if file.is_file() and (file.suffix in (".so", ".dylib", ".real") or file.name == "Python"):
        subprocess.run(["codesign", "--force", "--sign", "-", str(file)], check=True, capture_output=True)
for formula in json.loads(output("brew", "info", "--json=v2", "--installed"))["formulae"]:
    name = formula["name"]
    installed = Path(output("brew", "--prefix", name))
    for notice in installed.glob("*"):
        if notice.is_file() and notice.name.lower().startswith(("license", "copying", "copyright")):
            shutil.copy2(notice, root / "licenses" / (name + "-" + notice.name))
(root / "licenses/homebrew.json").write_text(output("brew", "info", "--json=v2", "--installed"))
