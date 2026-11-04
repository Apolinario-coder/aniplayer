import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile

root = Path(sys.argv[1])
client, child = socket.socketpair()
client.settimeout(10)
process = subprocess.Popen([
    str(root / "bin/mpv"), "--idle=yes", "--terminal=yes", "--config=no",
    "--input-ipc-client=fd://0", "--vo=null", "--ao=null",
    "--hwdec=auto-copy", "--panscan=1", "--ytdl=yes",
], stdin=child, stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
child.close()
try:
    client.sendall(b'{"command":["get_property","idle-active"],"request_id":1}\n')
    reader = client.makefile("rb")
    for line in reader:
        response = json.loads(line)
        if response.get("request_id") == 1:
            assert response["error"] == "success", response
            print("Bundled mpv IPC is ready.")
            break
    else:
        raise RuntimeError("mpv closed IPC during startup")
except Exception:
    process.terminate()
    print(process.communicate(timeout=10)[0].decode("utf-8", "replace"))
    raise
finally:
    client.close()
    process.terminate()
    process.wait(timeout=10)

with tempfile.TemporaryDirectory() as folder:
    clip = Path(folder) / "test.mp4"
    script = Path(folder) / "test.vpy"
    script.write_text("video_in.std.FlipVertical().set_output()\n")
    subprocess.run([str(root / "bin/ffmpeg"), "-v", "error", "-f", "lavfi",
                    "-i", "color=s=64x64:r=24", "-t", "0.2", str(clip)], check=True)
    path = str(script)
    result = subprocess.run([
        str(root / "bin/mpv"), "--config=no", "--vo=null", "--ao=null", "--frames=2",
        f"--vf=vapoursynth=file=%{len(path.encode('utf-8'))}%{path}", str(clip),
    ], capture_output=True, timeout=30)
    if result.returncode:
        raise RuntimeError(result.stdout.decode("utf-8", "replace") + result.stderr.decode("utf-8", "replace"))
    print("Bundled mpv processed video through VapourSynth.")
