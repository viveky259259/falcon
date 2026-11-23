"""Owns one falcon-mcp process and relays tool calls sent over a unix socket."""
import json, os, socket, subprocess, sys, time
sock_path, binary, transcript = sys.argv[1], sys.argv[2], sys.argv[3]
proc = subprocess.Popen([binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
next_id = 0
def rpc(method, params, notify=False):
    global next_id
    msg = {"jsonrpc": "2.0", "method": method, "params": params}
    if not notify:
        next_id += 1
        msg["id"] = next_id
    proc.stdin.write(json.dumps(msg) + "\n"); proc.stdin.flush()
    if notify:
        return None
    while True:
        line = proc.stdout.readline()
        if not line:
            return {"error": "falcon-mcp exited"}
        reply = json.loads(line)
        if reply.get("id") == next_id:
            return reply
init = rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "loop-proof", "version": "1"}})
rpc("notifications/initialized", {}, notify=True)
with open(transcript, "a") as f:
    f.write(json.dumps({"initialize": init.get("result", init)}) + "\n")
if os.path.exists(sock_path):
    os.remove(sock_path)
server = socket.socket(socket.AF_UNIX); server.bind(sock_path); server.listen(1)
while True:
    conn, _ = server.accept()
    data = b""
    while not data.endswith(b"\n"):
        chunk = conn.recv(65536)
        if not chunk: break
        data += chunk
    req = json.loads(data)
    if req.get("quit"):
        conn.sendall(b"{}\n"); conn.close(); break
    started = time.time()
    reply = rpc("tools/call", {"name": req["tool"], "arguments": req["args"]})
    entry = {"tool": req["tool"], "arguments": req["args"], "seconds": round(time.time() - started, 1), "reply": reply.get("result", reply)}
    with open(transcript, "a") as f:
        f.write(json.dumps(entry) + "\n")
    conn.sendall((json.dumps(entry) + "\n").encode()); conn.close()
proc.stdin.close(); proc.wait(timeout=30)
