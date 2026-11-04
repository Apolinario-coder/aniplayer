import json
import os
from pathlib import Path
import socket
import subprocess
import sys

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
