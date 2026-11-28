"""mcp_call.py <socket> <tool> '<json args>' : call one falcon-mcp tool and print the reply."""
import json, socket, sys
s = socket.socket(socket.AF_UNIX); s.connect(sys.argv[1])
req = {"quit": True} if sys.argv[2] == "--quit" else {"tool": sys.argv[2], "args": json.loads(sys.argv[3] if len(sys.argv) > 3 else "{}")}
s.sendall((json.dumps(req) + "\n").encode())
data = b""
while not data.endswith(b"\n"):
    chunk = s.recv(1 << 20)
    if not chunk: break
    data += chunk
entry = json.loads(data)
reply = entry.get("reply", {})
print(f"# {entry.get('tool')} {json.dumps(entry.get('arguments'))}  ({entry.get('seconds')}s)  isError={reply.get('isError', False)}")
for block in reply.get("content", []):
    if block.get("type") == "text":
        print(block["text"])
    else:
        print(f"[{block.get('type')} block, {len(block.get('data', ''))} base64 chars]")
if "content" not in reply:
    print(json.dumps(reply, indent=2))
