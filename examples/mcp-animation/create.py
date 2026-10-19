"""Create the animation through stdio MCP; export with Pixloom's existing CLI.

No dependencies beyond Python 3. --preview keeps the MCP connection alive until
Ctrl+C so its read-only windows remain available. Existing artwork is never replaced.
"""
import argparse
import base64
import json
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--pixloom", type=Path, required=True)
parser.add_argument("--output", type=Path, required=True)
parser.add_argument("--preview", action="store_true")
parser.add_argument("--bridge", type=Path, help=argparse.SUPPRESS)
args = parser.parse_args()
exe = args.pixloom.resolve()
mcp = exe.with_name("pixloom-mcp" + exe.suffix)
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=True)
project = out / "bounce.pixloom.json"
if project.exists():
    raise SystemExit(f"Refusing to replace {project}; choose a fresh output directory")
log = (out / "mcp-stderr.log").open("w")
server = subprocess.Popen([str(mcp)], stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, stderr=log, text=True,
                          creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
request_id = 0
transcript = []

def request(method, params):
    global request_id
    request_id += 1
    server.stdin.write(json.dumps(dict(jsonrpc="2.0", id=request_id, method=method, params=params)) + "\n")
    server.stdin.flush()
    while True:
        line = server.stdout.readline()
        if not line:
            raise RuntimeError("MCP server disconnected; inspect mcp-stderr.log")
        reply = json.loads(line)
        if reply.get("id") == request_id:
            if "error" in reply:
                raise RuntimeError(reply["error"])
            return reply["result"]

def call(name, arguments, image_path=None):
    reply = request("tools/call", dict(name=name, arguments=arguments))
    if reply.get("isError"):
        raise RuntimeError(reply)
    metadata = None
    for content in reply["content"]:
        if content["type"] == "text":
            metadata = json.loads(content["text"])
        elif content["type"] == "image" and image_path:
            image_path.write_bytes(base64.b64decode(content["data"]))
    transcript.append(dict(tool=name, arguments=arguments, result=metadata))
    return metadata

try:
    request("initialize", dict(protocolVersion="2025-11-25", capabilities={},
                               clientInfo=dict(name="pixloom-animation-example", version="1")))
    server.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
    server.stdin.flush()
    created = call("create_project", dict(width=24, height=24,
                   palette=["#00000000", "#15243AFF", "#36A9D6FF", "#8CE7E5FF", "#F6E6A6FF", "#42536CFF"],
                   output_path=str(project)))
    pid = created["project_id"]
    revision = created["revision"]

    def batch(operations):
        global revision
        revision = call("apply_edits", dict(project_id=pid, expected_revision=revision,
                                             operations=operations))["revision"]

    # Block out a squash pose; review before creating the remaining poses.
    batch([dict(type="rename_layer", layer_id="layer-1", name="Orb"),
           dict(type="add_layer", layer_id="ground", name="Ground", index=0),
           dict(type="set_frame_duration", frame_id="frame-1", duration_ms=200),
           dict(type="rectangle", frame_id="frame-1", layer_id="ground", x=3, y=21,
                width=18, height=1, palette_index=5, filled=True),
           dict(type="ellipse", frame_id="frame-1", layer_id="layer-1", x=7, y=15,
                width=10, height=6, palette_index=1, filled=True),
           dict(type="ellipse", frame_id="frame-1", layer_id="layer-1", x=8, y=16,
                width=8, height=4, palette_index=2, filled=True)])
    call("inspect_project", dict(project_id=pid, frame_id="frame-1", scale=10), out / "blockout.png")
    # Six distinct poses, matching ascent/descent with unequal holds and squash/stretch.
    poses = [("frame-1", 200, 7, 15, 10, 6), ("rise", 100, 9, 10, 6, 9),
             ("apex-in", 140, 8, 4, 8, 8), ("apex-out", 220, 8, 3, 8, 8),
             ("fall", 90, 9, 8, 6, 10), ("land", 120, 8, 13, 8, 8)]
    for index, (frame, duration, *_) in enumerate(poses[1:], start=1):
        revision = call("edit_frame", dict(project_id=pid, expected_revision=revision,
                         operation=dict(type="duplicate_frame", frame_id="frame-1", new_frame_id=frame, index=index)))["revision"]
        revision = call("edit_frame", dict(project_id=pid, expected_revision=revision,
                         operation=dict(type="set_frame_duration", frame_id=frame, duration_ms=duration)))["revision"]
    operations = []
    for frame, duration, x, y, w, h in poses:
        common = dict(frame_id=frame, layer_id="layer-1")
        operations += [dict(type="clear_cel", **common),
            dict(type="ellipse", x=x, y=y, width=w, height=h, palette_index=1, filled=True, **common),
            dict(type="ellipse", x=x+1, y=y+1, width=w-2, height=h-2, palette_index=2, filled=True, **common),
            dict(type="line", x1=x+2, y1=y+1, x2=x+w-3, y2=y+1, palette_index=3, **common),
            dict(type="set_pixels", pixels=[dict(x=x+w-3, y=y+max(2,h//2), palette_index=4)], **common)]
    batch(operations)
    sheet = call("get_contact_sheet", dict(project_id=pid, scale=8, columns=3), out / "contact-sheet.png")
    assert sheet["distinct_composites"] == len(poses)
    # Targeted correction: a second eye on the stretched falling pose only.
    batch([dict(type="set_pixels", frame_id="fall", layer_id="layer-1",
                pixels=[dict(x=10, y=13, palette_index=4)])])
    for frame, *_ in poses:
        call("inspect_project", dict(project_id=pid, frame_id=frame, scale=10), out / f"{frame}.png")
    call("get_contact_sheet", dict(project_id=pid, scale=8, columns=3), out / "contact-sheet.png")
    info = call("get_project_info", dict(project_id=pid))
    call("finish_project", dict(project_id=pid))
    # Export reloads and validates the saved editable source; it does not invoke CLI edits.
    subprocess.run([str(exe), "validate", str(project)], check=True)
    subprocess.run([str(exe), "export", str(project), "--output", str(out / "bounce"),
                    "--formats", "gif", "--loop"], check=True)
    (out / "tool-transcript.json").write_text(json.dumps(transcript, indent=2))
    (out / "review-metadata.json").write_text(json.dumps(info, indent=2))
    print(f"Created {project} and {out / 'bounce.gif'}", flush=True)
    if args.preview:
        for path in [project, out / "bounce.gif"]:
            print(json.dumps(call("open_preview", dict(path=str(path)))), flush=True)
        if args.bridge:
            bridge = args.bridge.resolve()
            bridge.mkdir(parents=True, exist_ok=True)
            (bridge / "ready.json").write_text(json.dumps(dict(server_pid=server.pid, project=str(project))))
            previous = None
            while server.poll() is None:
                req = bridge / "request.json"
                if req.exists():
                    message = json.loads(req.read_text())
                    if message["id"] != previous:
                        previous = message["id"]
                        if message["name"] == "_shutdown":
                            break
                        try:
                            result = call(message["name"], message["arguments"])
                        except Exception as e:
                            result = dict(error=str(e))
                        (bridge / "response.json").write_text(json.dumps(dict(id=previous, result=result)))
                time.sleep(0.1)
        else:
            print("Preview connection remains open. Press Ctrl+C to close its windows.", flush=True)
            while server.poll() is None:
                time.sleep(0.25)
except KeyboardInterrupt:
    pass
finally:
    if server.stdin:
        server.stdin.close()
    server.wait(timeout=10)
    log.close()
