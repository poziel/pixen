"""Exercise report_activity through real stdio MCP. Python standard library only."""
import argparse
import json
from pathlib import Path
import queue
import subprocess
import threading
import time
from datetime import datetime

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", type=Path)
parser.add_argument("--delay", type=float, default=2.5, help="Seconds between visible steps")
parser.add_argument("--hold", type=float, default=300, help="Seconds to keep the preview open afterward")
parser.add_argument("--headless", action="store_true", help="Verify the endpoint without opening a window")
args = parser.parse_args()
out = (args.output or ROOT / "out" / ("mcp-activity-" + datetime.now().strftime("%Y%m%d-%H%M%S-%f"))).resolve()
out.mkdir(parents=True, exist_ok=True)
project = out / "ranger-weapons.pixloom.json"
if project.exists():
    raise SystemExit(f"Refusing to replace {project}")
log = (out / "mcp-stderr.log").open("w")
server = subprocess.Popen([str(ROOT / "target/debug/pixen-mcp.exe")],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=log, text=True,
    creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
replies = queue.Queue()
def read_replies():
    try:
        for line in server.stdout:
            replies.put(json.loads(line))
    finally:
        replies.put(None)
threading.Thread(target=read_replies, daemon=True).start()
sequence = 0
transcript = []
def request(method, params):
    global sequence
    sequence += 1
    server.stdin.write(json.dumps({"jsonrpc":"2.0", "id":sequence, "method":method, "params":params}) + "\n")
    server.stdin.flush()
    while True:
        reply = replies.get(timeout=20)
        if reply is None:
            raise RuntimeError("MCP disconnected; inspect mcp-stderr.log")
        if reply.get("id") == sequence:
            if "error" in reply:
                raise RuntimeError(reply["error"])
            return reply["result"]
def call(name, **arguments):
    reply = request("tools/call", {"name":name, "arguments":arguments})
    if reply.get("isError"):
        raise RuntimeError(reply)
    value = next(json.loads(item["text"]) for item in reply["content"] if item["type"] == "text")
    transcript.append({"tool":name, "arguments":arguments, "result":value})
    (out / "transcript.json").write_text(json.dumps(transcript, indent=2))
    return value
def pause():
    if not args.headless:
        time.sleep(max(0, args.delay))
try:
    request("initialize", {"protocolVersion":"2025-11-25", "capabilities":{},
        "clientInfo":{"name":"pixen-activity-example", "version":"1"}})
    server.stdin.write('{"jsonrpc":"2.0","method":"notifications/initialized"}\n')
    server.stdin.flush()
    tools = request("tools/list", {})
    assert any(tool["name"] == "report_activity" for tool in tools["tools"])
    created = call("create_project", width=24, height=24,
        palette=["#00000000", "#BDDCEBFF", "#50647AFF", "#B1814FFF", "#74BB88FF"],
        output_path=str(project))
    pid = created["project_id"]
    revision = 0
    if not args.headless:
        call("open_preview", project_id=pid)
    def activity(message, busy=True, frame="frame-1", layer="layer-1"):
        before = revision
        result = call("report_activity", project_id=pid, message=message, busy=busy,
            frame_id=frame, layer_id=layer)
        assert result["revision"] == before, "Activity must never increment revision"
        pause()
    def edit(operations):
        global revision
        result = call("apply_edits", project_id=pid, expected_revision=revision, operations=operations)
        revision = result["revision"]
        pause()
    activity("Preparing the ranger's weapons")
    edit([{"type":"rename_layer", "layer_id":"layer-1", "name":"Sword"},
          {"type":"add_layer", "layer_id":"bow", "name":"Bow"},
          {"type":"add_frame", "frame_id":"frame-2", "duration_ms":180},
          {"type":"add_frame", "frame_id":"frame-3", "duration_ms":180}])
    for index in range(3):
        frame = f"frame-{index+1}"
        activity(f"Drawing the sword · pose {index+1}", frame=frame)
        x = 5 + index
        common = {"frame_id":frame, "layer_id":"layer-1"}
        edit([{"type":"rectangle", **common, "x":x, "y":3, "width":3, "height":12,
               "palette_index":1, "filled":True},
              {"type":"line", **common, "x1":x, "y1":3, "x2":x, "y2":14, "palette_index":2},
              {"type":"rectangle", **common, "x":x-2, "y":15, "width":7, "height":2,
               "palette_index":3, "filled":True},
              {"type":"rectangle", **common, "x":x+1, "y":17, "width":1, "height":4,
               "palette_index":3, "filled":True}])
        activity(f"Drawing the bow · pose {index+1}", frame=frame, layer="bow")
        common["layer_id"] = "bow"
        edit([{"type":"line", **common, "x1":16, "y1":3, "x2":20, "y2":11+index, "palette_index":3},
              {"type":"line", **common, "x1":20, "y1":11+index, "x2":16, "y2":21, "palette_index":3},
              {"type":"line", **common, "x1":16, "y1":3, "x2":16, "y2":21, "palette_index":1}])
    # Revisit targets slowly so switching Follow AI off/on is easy to inspect.
    for index in range(6):
        activity("Reviewing the ranger's weapons", frame=f"frame-{index%3+1}",
            layer="bow" if index%2 else "layer-1")
    activity("Ranger weapons ready", busy=False, frame="frame-3", layer="bow")
    assert call("get_project_info", project_id=pid)["revision"] == revision
    if not args.headless:
        time.sleep(max(0, args.hold))
    call("finish_project", project_id=pid)
    (out / "result.json").write_text(json.dumps({"passed":True, "revision":revision,
        "project":str(project), "report_activity_calls":sum(t["tool"]=="report_activity" for t in transcript)}, indent=2))
    print(f"PASS: {out}")
finally:
    server.stdin.close()
    try:
        server.wait(timeout=5)
    except subprocess.TimeoutExpired:
        server.terminate()
        server.wait(timeout=5)
    log.close()
