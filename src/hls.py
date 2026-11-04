from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import re
import sys


class HlsHandler(SimpleHTTPRequestHandler):
    extensions_map = {".m3u8": "application/vnd.apple.mpegurl", ".ts": "video/mp2t"}

    def send_head(self):
        if not re.fullmatch(r"/(?:live\.m3u8|segment-[0-9]+\.ts)", self.path):
            self.send_error(404)
            return None
        return super().send_head()

    def end_headers(self):
        self.send_header("Cache-Control", "no-cache")
        super().end_headers()

    def log_message(self, *args):
        pass


ThreadingHTTPServer((sys.argv[1], int(sys.argv[2])), HlsHandler).serve_forever()
