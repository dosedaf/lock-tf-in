#!/usr/bin/env python3
"""Minimal MCP stdio server for LOCK IN.

Exposes lockinctl as MCP tools so an agent (or voice transcription feeding an
agent) can run: start/pause the timer, add a task, log time, update tracker
progress, read the summary.

Configure with:
  LOCKIN_CTL=/home/yoda/.local/bin/lockinctl
Set LOCKIN_DB if the database is not the default.
"""
import json
import os
import subprocess
import sys
from pathlib import Path

TOOLS = [
    {
        "name": "lockin_task_add",
        "description": "Add a task to the current day.",
        "inputSchema": {
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"],
        },
    },
    {
        "name": "lockin_task_state",
        "description": "Set a task state: todo, doing or done.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "id": {"type": "integer"},
                "state": {"type": "string", "enum": ["todo", "doing", "done"]},
            },
            "required": ["id", "state"],
        },
    },
    {
        "name": "lockin_task_list",
        "description": "List tasks and their states.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "lockin_timer",
        "description": "Control the focus timer: start, pause, reset, skip or status.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["start", "pause", "reset", "skip", "status"],
                }
            },
            "required": ["action"],
        },
    },
    {
        "name": "lockin_log_time",
        "description": "Log a manual time period.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "minutes": {"type": "integer", "minimum": 1, "maximum": 1440},
                "category": {"type": "string"},
                "subject_id": {"type": ["integer", "null"]},
                "date": {"type": "string", "description": "YYYY-MM-DD"},
                "start": {"type": "string", "description": "HH:MM"},
            },
            "required": ["minutes"],
        },
    },
    {
        "name": "lockin_tracker_progress",
        "description": "Change tracker progress by delta or set an absolute value.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "subject_id": {"type": "integer"},
                "delta": {"type": "number"},
                "set": {"type": "number"},
            },
            "required": ["subject_id"],
        },
    },
    {
        "name": "lockin_tracker_state",
        "description": "Set a tracker subject state.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "subject_id": {"type": "integer"},
                "state": {
                    "type": "string",
                    "enum": ["todo", "doing", "paused", "done", "dropped"],
                },
            },
            "required": ["subject_id", "state"],
        },
    },
    {
        "name": "lockin_tracker_list",
        "description": "List tracker groups and subjects.",
        "inputSchema": {"type": "object", "properties": {}},
    },
    {
        "name": "lockin_summary",
        "description": "Read focused-time totals and recent daily breakdown.",
        "inputSchema": {
            "type": "object",
            "properties": {"days": {"type": "integer"}},
        },
    },
]


def ctl_path() -> str:
    explicit = os.environ.get("LOCKIN_CTL", "").strip()
    if explicit:
        return explicit
    home = Path.home()
    candidates = [
        home / ".local/bin/lockinctl",
        Path(__file__).resolve().parent.parent / "target/release/lockinctl",
        Path(__file__).resolve().parent.parent / "target/debug/lockinctl",
    ]
    for c in candidates:
        if c.exists():
            return str(c)
    return "lockinctl"


def run_ctl(args, timeout=20):
    cmd = [ctl_path()]
    db = os.environ.get("LOCKIN_DB", "").strip()
    if db:
        cmd += ["--db", db]
    cmd += args
    try:
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
    except FileNotFoundError:
        return True, f"lockinctl not found at {ctl_path()}"
    except subprocess.TimeoutExpired:
        return True, "lockinctl timed out"
    out = (p.stdout or "").strip()
    err = (p.stderr or "").strip()
    return p.returncode != 0, (err or out or f"exit {p.returncode}")


def call_tool(name, args):
    if name == "lockin_task_add":
        return run_ctl(["task", "add", str(args.get("text", ""))])
    if name == "lockin_task_state":
        return run_ctl(["task", str(args["state"]), str(int(args["id"]))])
    if name == "lockin_task_list":
        return run_ctl(["task", "list"])
    if name == "lockin_timer":
        return run_ctl(["timer", str(args.get("action", "status"))])
    if name == "lockin_log_time":
        a = ["log", str(int(args["minutes"]))]
        if args.get("category"):
            a += ["--category", str(args["category"])]
        if args.get("subject_id") is not None:
            a += ["--subject", str(int(args["subject_id"]))]
        if args.get("date"):
            a += ["--date", str(args["date"])]
        if args.get("start"):
            a += ["--start", str(args["start"])]
        return run_ctl(a)
    if name == "lockin_tracker_progress":
        amount = f"set:{args['set']}" if args.get("set") is not None else str(args.get("delta", 0))
        return run_ctl(["tracker", "progress", str(int(args["subject_id"])), amount])
    if name == "lockin_tracker_state":
        return run_ctl(
            ["tracker", "state", str(int(args["subject_id"])), str(args["state"])]
        )
    if name == "lockin_tracker_list":
        return run_ctl(["tracker", "list"])
    if name == "lockin_summary":
        a = ["summary"]
        if args.get("days") is not None:
            a += ["--days", str(int(args["days"]))]
        return run_ctl(a)
    return True, f"unknown tool {name}"


def send(msg):
    sys.stdout.write(json.dumps(msg, separators=(",", ":")) + "\n")
    sys.stdout.flush()


def read_message():
    line = sys.stdin.readline()
    if not line:
        return None
    if line.lower().startswith("content-length:"):
        length = int(line.split(":", 1)[1].strip())
        while True:
            h = sys.stdin.readline()
            if h in ("\r\n", "\n", ""):
                break
        body = sys.stdin.read(length)
        return json.loads(body)
    line = line.strip()
    return json.loads(line) if line else None


def main():
    while True:
        try:
            req = read_message()
        except Exception:
            continue
        if req is None:
            break
        method = req.get("method")
        req_id = req.get("id")
        if method == "initialize":
            send(
                {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "protocolVersion": req.get("params", {}).get(
                            "protocolVersion", "2024-11-05"
                        ),
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "lockin", "version": "1.0.0"},
                    },
                }
            )
        elif method == "notifications/initialized":
            continue
        elif method == "ping":
            send({"jsonrpc": "2.0", "id": req_id, "result": {}})
        elif method == "tools/list":
            send({"jsonrpc": "2.0", "id": req_id, "result": {"tools": TOOLS}})
        elif method == "tools/call":
            params = req.get("params", {})
            name = params.get("name", "")
            args = params.get("arguments", {}) or {}
            is_error, text = call_tool(name, args)
            send(
                {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "result": {
                        "content": [{"type": "text", "text": text}],
                        "isError": is_error,
                    },
                }
            )
        elif req_id is not None:
            send(
                {
                    "jsonrpc": "2.0",
                    "id": req_id,
                    "error": {"code": -32601, "message": f"unknown method {method}"},
                }
            )


if __name__ == "__main__":
    main()
