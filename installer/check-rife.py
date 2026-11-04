import os
import sys
from pathlib import Path
import vapoursynth as vs

root = Path(sys.argv[1])
plugin = root / "rife" / ("librife.dylib" if sys.platform == "darwin" else "librife.so")
vs.core.std.LoadPlugin(str(plugin))
assert callable(vs.core.rife.RIFE)
assert (root / "rife/models/rife-v4.16lite/flownet.bin").stat().st_size > 0
print("Bundled VapourSynth loaded the RIFE plug-in.")
if os.environ.get("ANI_TEST_RIFE_FRAMES") == "1":
    clip = vs.core.std.BlankClip(width=64, height=64, length=2, fpsnum=24, format=vs.RGBS)
    result = vs.core.rife.RIFE(
        clip, model_path=str(root / "rife/models/rife-v4.16lite"), factor_num=2, factor_den=1)
    with result.get_frame(1):
        pass
    print("RIFE generated an intermediate frame.")
