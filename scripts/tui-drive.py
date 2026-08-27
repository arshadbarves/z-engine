import os, pty, select, subprocess, sys, time, re, fcntl, termios, struct

def strip_ansi(b):
    return re.sub(rb"\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*\x07|\x1b[()][B0]", b"", b)

mock = subprocess.Popen([sys.executable, "scripts/mock_llm_interactive.py"])
for _ in range(50):
    if os.path.exists("/tmp/mock_port"):
        break
    time.sleep(0.05)
time.sleep(0.15)

master, slave = pty.openpty()
# ratatui needs a non-zero window size
fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 100, 0, 0))
env = dict(os.environ, ZENGINE_API_KEY="mock", HARNESS_API_KEY="mock", TERM="xterm-256color")
proc = subprocess.Popen(
    ["target/debug/zengine", "--base-url", f"http://127.0.0.1:{open('/tmp/mock_port').read().strip()}/v1"],
    stdin=slave, stdout=slave, stderr=slave, env=env, close_fds=True,
)
os.close(slave)

buf = bytearray()
def read_for(seconds):
    end = time.time() + seconds
    while time.time() < end:
        r, _, _ = select.select([master], [], [], 0.15)
        if r:
            try:
                data = os.read(master, 65536)
            except OSError:
                break
            if not data:
                break
            buf.extend(data)
    return strip_ansi(bytes(buf))

screen = read_for(2.5)
assert b"type a task" in screen or b"zengine v" in screen, f"no startup UI:\n{screen[-800:]!r}"
print("[drive] startup UI ok")

if proc.poll() is not None:
    print(f"zengine exited early rc={proc.returncode}")
    raise SystemExit(3)
os.write(master, b"create the proof file\r")
screen = read_for(4.0)
assert b"approval required" in screen, f"no approval modal:\n{screen[-1200:]!r}"
print("[drive] approval modal appeared")
assert b"touch /tmp/interactive-proof" in screen
print("[drive] modal shows command preview")

os.write(master, b"y")   # yes once
screen = read_for(4.0)
assert b"approved once" in screen, f"no approved notice:\n{screen[-1200:]!r}"
print("[drive] approved once")

# wait for completion marker
deadline = time.time() + 10
while time.time() < deadline:
    screen = read_for(1.0)
    if b"\xe2\x9c\x93 done".decode().encode() in bytes(buf) or b"done" in screen[-400:]:
        break
raw = bytes(buf)
final = strip_ansi(raw)
collapsed = re.sub(rb"\s+", b"", final)
assert b"Interactiveflowcomplete." in collapsed, f"no final answer:\n{final[-1500:]!r}"
print("[drive] turn completed with model answer")

assert os.path.exists("/tmp/interactive-proof"), "tool never actually ran"
print("[drive] side-effect verified on disk")

# quit: double Ctrl-C
os.write(master, b"\x03")
time.sleep(0.4)
os.write(master, b"\x03")
try:
    rc = proc.wait(timeout=5)
except subprocess.TimeoutExpired:
    proc.kill(); rc = -9
print(f"[drive] exit code: {rc}")
assert rc == 0, "TUI did not exit cleanly"

os.close(master)
mock.terminate()
print("PTY-DRIVE PASSED")
