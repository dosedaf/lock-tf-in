# LOCK IN MCP / voice control

`lockin_mcp.py` exposes `lockinctl` as MCP tools over stdio:

- `lockin_task_add`, `lockin_task_state`, `lockin_task_list`
- `lockin_timer` (start/pause/reset/skip/status)
- `lockin_log_time`
- `lockin_tracker_progress`, `lockin_tracker_state`, `lockin_tracker_list`
- `lockin_summary`

## Agent config

```json
{
  "mcpServers": {
    "lockin": {
      "command": "/home/yoda/.local/bin/lockin-mcp",
      "env": {
        "LOCKIN_CTL": "/home/yoda/.local/bin/lockinctl"
      }
    }
  }
}
```

## Voice

Point your dictation/STT layer at the same agent. Utterances map directly:

- “start the timer” → `lockin_timer {action: "start"}`
- “log two hours to work today” → `lockin_log_time {minutes: 120, category: "work"}`
- “add a task finish the index rebuild” → `lockin_task_add {text: "finish the index rebuild"}`
- “twenty pages of DDIA” → `lockin_tracker_progress {subject_id: …, delta: 20}`
- “how much did I focus this week” → `lockin_summary {days: 7}`

Destructive actions (task delete, subject delete) are still confirmation-only in the UI.
