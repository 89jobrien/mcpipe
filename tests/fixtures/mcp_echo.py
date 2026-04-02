#!/usr/bin/env python3
"""Minimal MCP stdio server for testing.
Responds to tools/list and tools/call over stdin/stdout (JSON-RPC 2.0).
"""
import json
import sys

TOOLS = [
    {
        "name": "echo",
        "description": "Echo the input back",
        "inputSchema": {
            "type": "object",
            "required": ["message"],
            "properties": {
                "message": {"type": "string", "description": "Text to echo"}
            }
        }
    }
]

def handle(req):
    method = req.get("method", "")
    rid = req.get("id")

    if method == "initialize":
        return {"jsonrpc": "2.0", "id": rid, "result": {
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "echo-server", "version": "0.1.0"}
        }}
    if method == "tools/list":
        return {"jsonrpc": "2.0", "id": rid, "result": {"tools": TOOLS}}
    if method == "tools/call":
        tool = req.get("params", {}).get("name", "")
        args = req.get("params", {}).get("arguments", {})
        if tool == "echo":
            return {"jsonrpc": "2.0", "id": rid, "result": {
                "content": [{"type": "text", "text": args.get("message", "")}]
            }}
        return {"jsonrpc": "2.0", "id": rid, "error": {"code": -32601, "message": "tool not found"}}
    if method == "notifications/initialized":
        return None  # no response for notifications
    return {"jsonrpc": "2.0", "id": rid, "error": {"code": -32601, "message": "method not found"}}

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        req = json.loads(line)
    except json.JSONDecodeError:
        continue
    resp = handle(req)
    if resp is not None:
        print(json.dumps(resp), flush=True)
