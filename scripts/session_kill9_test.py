#!/usr/bin/env python3
"""v0.4 acceptance: kill -9 mid-task, restart with --resume, continue.

Phase HANG: round 1 completes (tool result recorded), then the mock holds
the second request open forever -> we SIGKILL harness mid-turn.
Phase FINISH: relaunch with --session; mock answers immediately; assert the
resumed model request contains the pre-crash context.
"""
import json, os, signal, socket, subprocess, sys, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ROOT = os.path.dirname(os.path.abspath(__file__)) + "/.."
STATE = {"phase": "hang", "bodies": []}
SECRET = "PHOENIX_KEY=ZX99"

def sse(obj): return f"data: {json.dumps(obj)}\n\n".encode()

class Reuse(ThreadingHTTPServer):
    allow_reuse_address = True

class H(BaseHTTPRequestHandler):
    def log_message(self, *a): pass
    def do_POST(self):
        n = int(self.headers.get("content-length", 0))
        body = self.rfile.read(n).decode()
        STATE["bodies"].append(body)
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.end_headers()

        if STATE["phase"] == "finish":
            chunks = [
                {"choices": [{"index": 0, "delta": {"content": "resumed ok — key is ZX99"}}]},
                {"choices": [{"index": 0, "delta": {}, "finish_reason": "stop"}],
                 "usage": {"prompt_tokens": 50, "completion_tokens": 5}},
                "data: [DONE]\n\n",
            ]
            for c in chunks:
                self.wfile.write(sse(c) if isinstance(c, dict) else c.encode())
                self.wfile.flush()
            return

        count = len(STATE["bodies"])
        if count == 1:
            # round 1: read a file whose content carries the secret marker
            for c in [
                {"choices": [{"index": 0, "delta": {"content": "searching"}}]},
                {"choices": [{"index": 0, "delta": {"tool_calls": [
                    {"index": 0, "id": "k1", "type": "function",
                     "function": {"name": "bash", "arguments": ""}}]}}]},
                {"choices": [{"index": 0, "delta": {"tool_calls": [
                    {"index": 0, "function": {"arguments": json.dumps({"command": "echo " + SECRET})}}]}}]},
                {"choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}],
                 "usage": {"prompt_tokens": 30, "completion_tokens": 10}},
                "data: [DONE]\n\n",
            ]:
                self.wfile.write(sse(c) if isinstance(c, dict) else c.encode())
                self.wfile.flush()
            return

        # second+ request in hang phase: hold the socket open forever
        print("[mock] hanging to simulate stall", flush=True)
        time.sleep(3600)

srv = Reuse(("127.0.0.1", 0), H)
port = srv.server_address[1]
open("/tmp/v04_port", "w").write(str(port))

import threading
threading.Thread(target=srv.serve_forever, daemon=True).start()

workdir = os.path.join(ROOT, "tmp/acceptance-v01")
import platform
if platform.system() == "Darwin":
    sessions_dir = os.path.expanduser("~/Library/Application Support/z-engine/sessions")
    legacy = os.path.expanduser("~/Library/Application Support/harness/sessions")
else:
    share = os.environ.get("XDG_DATA_HOME", os.path.expanduser("~/.local/share"))
    sessions_dir = share + "/z-engine/sessions"
    legacy = share + "/harness/sessions"
if not os.path.isdir(sessions_dir) and os.path.isdir(legacy):
    sessions_dir = legacy

def newest_session():
    files = [os.path.join(sessions_dir, f) for f in os.listdir(sessions_dir)]
    return max(files, key=os.path.getmtime)

def wait_for(predicate, timeout=30, what=""):
    end = time.time() + timeout
    while time.time() < end:
        v = predicate()
        if v:
            return v
        time.sleep(0.15)
    raise SystemExit(f"timeout waiting for {what}")

env = dict(os.environ, ZENGINE_API_KEY="mock", HARNESS_API_KEY="mock")
base = f"http://127.0.0.1:{port}/v1"
binpath = os.path.join(ROOT, "target/debug/zengine")

# ---- phase HANG: start, let round 1 record, then kill -9 -------------------
p1 = subprocess.Popen(
    [binpath, "--headless", "find the phoenix key", "--project", workdir,
     "--base-url", base, "--auto-approve"],
    env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

def session_has_round1():
    try:
        p = newest_session()
        txt = open(p).read()
        return "PHOENIX_KEY" in txt and '"type":"assistant_msg"' in txt
    except Exception:
        return False

wait_for(session_has_round1, what="round-1 transcript events")
time.sleep(1.5)  # ensure we're stuck on the hung second request
os.kill(p1.pid, signal.SIGKILL)
p1.wait()
print("[accept] killed -9 mid-task; session file kept transcript")
session_file = newest_session()

# ---- phase FINISH: resume --------------------------------------------------
STATE["phase"] = "finish"
STATE["bodies"].clear()
p2 = subprocess.Popen(
    [binpath, "--headless", "continue", "--project", workdir,
     "--base-url", base, "--auto-approve", "--session", session_file],
    env=env, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
try:
    out, _ = p2.communicate(timeout=60)
except subprocess.TimeoutExpired:
    p2.kill(); raise
assert p2.returncode == 0, f"resume run failed rc={p2.returncode}"
assert b"resumed ok" in out, f"no resumed answer: {out!r}"

# The first model request after resume must contain pre-crash context.
first_body = STATE["bodies"][0]
assert "find the phoenix key" in first_body, "original user task missing"
assert "PHOENIX_KEY" in first_body or "harness:tool-output" in first_body, \
    "pre-crash tool output missing from replayed context"
assert '"continue"' in first_body, "new user message missing"

print("SESSION KILL-9 ACCEPTANCE PASSED")
