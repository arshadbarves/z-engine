import json
from http.server import BaseHTTPRequestHandler, HTTPServer

ROUND = {"n": 0}

def sse(obj): return f"data: {json.dumps(obj)}\n\n".encode()

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_POST(self):
        n = int(self.headers.get("content-length", 0))
        self.rfile.read(n)
        ROUND["n"] += 1
        print(f"MOCK round {ROUND['n']}", flush=True)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()
        if ROUND["n"] == 1:
            # gated bash command (matches no allow rule -> approval modal)
            chunks = [
                {"choices":[{"index":0,"delta":{"content":"Creating proof file. "}}]},
                {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"g1","type":"function","function":{"name":"bash","arguments":""}}]}}]},
                {"choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{\"command\":\"touch /tmp/interactive-proof && echo created\"}"}}]}}]},
                {"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":40,"completion_tokens":10}},
                "data: [DONE]\n\n",
            ]
        else:
            chunks = [
                {"choices":[{"index":0,"delta":{"content":"Interactive flow complete."}}]},
                {"choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":80,"completion_tokens":20}},
                "data: [DONE]\n\n",
            ]
        for c in chunks:
            self.wfile.write(sse(c) if isinstance(c, dict) else c.encode())
            self.wfile.flush()

import socketserver
class Reuse(HTTPServer):
    allow_reuse_address = True
srv = Reuse(("127.0.0.1", 0), H)
open("/tmp/mock_port", "w").write(str(srv.server_address[1]))
srv.serve_forever()
