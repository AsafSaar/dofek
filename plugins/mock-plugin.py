"""Mock dofek plugin for testing the plugin system.

Responds to every poll with static panel data, a metric, and process annotations.
Usage: python plugins/mock-plugin.py
"""

import sys
import json

first = True
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue

    try:
        req = json.loads(line)
    except json.JSONDecodeError:
        continue

    if req.get("type") == "shutdown":
        break

    if req.get("type") != "poll":
        continue

    resp = {
        "status": "ok",
        "panels": [
            {
                "id": "mock-status",
                "label": "MOCK",
                "content": [
                    {"key": "Status", "value": "running", "style": "accent"},
                    {"key": "Uptime", "value": "42m", "style": "dim"},
                ],
            }
        ],
        "metrics": [
            {"id": "mock.test", "label": "Mock", "value": 42.0, "unit": ""}
        ],
        "process_annotations": [],
    }

    if first:
        resp["manifest"] = {
            "name": "mock-plugin",
            "version": "0.1.0",
            "description": "Test plugin for verifying the dofek plugin system",
            "author": "test",
        }
        first = False

    print(json.dumps(resp), flush=True)
