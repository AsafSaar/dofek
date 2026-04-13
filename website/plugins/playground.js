// dofek plugin playground — live JSON editor + dock preview

const EXAMPLE_RESPONSES = {
  ollama: `{
  "status": "ok",
  "manifest": {
    "name": "dofek-ollama",
    "version": "0.1.0",
    "description": "Ollama model status and inference tracking",
    "author": "dofek contributors"
  },
  "panels": [{
    "id": "ollama-status",
    "label": "OLLAMA",
    "content": [
      { "key": "Model", "value": "llama3:8b (4.7GB)", "style": "accent" },
      { "key": "Available", "value": "3 models", "style": "dim" }
    ]
  }],
  "process_annotations": [
    { "pid": 1234, "label": "llama3:8b", "category": "ai", "ai_state": "idle" }
  ],
  "metrics": [
    { "id": "ollama.running", "label": "Models", "value": 1.0, "unit": "" }
  ]
}`,
  docker: `{
  "status": "ok",
  "manifest": {
    "name": "dofek-docker",
    "version": "0.1.0",
    "description": "Docker container monitoring",
    "author": "dofek contributors"
  },
  "panels": [{
    "id": "docker-status",
    "label": "DOCKER",
    "content": [
      { "key": "postgres", "value": "postgres:16", "style": "accent" },
      { "key": "redis", "value": "redis:7-alpine", "style": "accent" },
      { "key": "nginx", "value": "nginx:latest", "style": "normal" }
    ]
  }],
  "process_annotations": [
    { "pid": 5678, "label": "3 containers", "category": "dev" }
  ],
  "metrics": [
    { "id": "docker.containers", "label": "Containers", "value": 3.0, "unit": "" }
  ]
}`,
  minimal: `{
  "status": "ok",
  "panels": [{
    "id": "my-plugin",
    "label": "MY PLUGIN",
    "content": [
      { "key": "Status", "value": "running", "style": "accent" }
    ]
  }],
  "process_annotations": [],
  "metrics": []
}`
};

const STYLE_COLORS = {
  normal: '#94a3b8',
  accent: '#38bdf8',
  dim: '#3d5070',
  warn: '#fbbf24',
  error: '#f87171'
};

const STATE_COLORS = {
  healthy: '#4ade80',
  starting: '#3d5070',
  unhealthy: '#fbbf24',
  crashed: '#f87171'
};

let currentExample = 'ollama';

function init() {
  const textarea = document.getElementById('pg-editor');
  const errorEl = document.getElementById('pg-error');

  // Load default example
  textarea.value = EXAMPLE_RESPONSES.ollama;
  updatePreview();

  // Live update on input
  textarea.addEventListener('input', updatePreview);

  // Example buttons
  document.querySelectorAll('[data-example]').forEach(btn => {
    btn.addEventListener('click', () => {
      const key = btn.dataset.example;
      currentExample = key;
      textarea.value = EXAMPLE_RESPONSES[key];

      document.querySelectorAll('[data-example]').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');

      updatePreview();
    });
  });

  // Scaffolder
  document.getElementById('scaffolder-generate')?.addEventListener('click', generateScaffold);
  document.getElementById('scaffolder-copy')?.addEventListener('click', copyScaffold);

  // Reveal animations
  const observer = new IntersectionObserver(entries => {
    entries.forEach(e => { if (e.isIntersecting) e.target.classList.add('visible'); });
  }, { threshold: 0.1 });
  document.querySelectorAll('.reveal').forEach(el => observer.observe(el));
}

function updatePreview() {
  const textarea = document.getElementById('pg-editor');
  const errorEl = document.getElementById('pg-error');
  const dockBody = document.getElementById('preview-dock-body');
  const tickerEl = document.getElementById('preview-ticker');

  let data;
  try {
    data = JSON.parse(textarea.value);
    errorEl.style.display = 'none';
  } catch (e) {
    errorEl.textContent = `Parse error: ${e.message}`;
    errorEl.style.display = 'block';
    dockBody.innerHTML = '';
    tickerEl.innerHTML = '';
    return;
  }

  // Validate structure
  const warnings = validate(data);
  if (warnings.length > 0) {
    errorEl.textContent = warnings.join(' · ');
    errorEl.style.display = 'block';
  }

  // Render dock preview
  renderDock(data, dockBody);

  // Render ticker preview
  renderTicker(data, tickerEl);
}

function validate(data) {
  const w = [];
  if (typeof data.status !== 'string') w.push('Missing "status" field');
  if (data.panels && !Array.isArray(data.panels)) w.push('"panels" must be an array');
  if (data.metrics && !Array.isArray(data.metrics)) w.push('"metrics" must be an array');
  if (data.process_annotations && !Array.isArray(data.process_annotations)) w.push('"process_annotations" must be an array');

  if (data.panels) {
    data.panels.forEach((p, i) => {
      if (!p.id) w.push(`panels[${i}]: missing "id"`);
      if (!p.label) w.push(`panels[${i}]: missing "label"`);
    });
  }
  if (data.metrics) {
    data.metrics.forEach((m, i) => {
      if (!m.id) w.push(`metrics[${i}]: missing "id"`);
      if (typeof m.value !== 'number') w.push(`metrics[${i}]: "value" must be a number`);
    });
  }
  return w;
}

function renderDock(data, container) {
  container.innerHTML = '';

  if (!data.panels || data.panels.length === 0) {
    container.innerHTML = '<div style="font-size:11px;color:#3d5070">No panels</div>';
    return;
  }

  const name = data.manifest?.name || data.panels[0]?.label || 'PLUGIN';

  const row = document.createElement('div');
  row.className = 'preview-dock-row';

  // Status dot
  const dot = document.createElement('span');
  dot.className = 'preview-dock-dot';
  dot.textContent = '●';
  dot.style.color = STATE_COLORS.healthy;
  row.appendChild(dot);

  // Plugin name
  const nameEl = document.createElement('span');
  nameEl.textContent = name.toUpperCase();
  nameEl.style.cssText = 'font-weight:700;color:#94a3b8;font-size:11px;letter-spacing:0.04em';
  row.appendChild(nameEl);

  // First panel content inline
  const panel = data.panels[0];
  if (panel.content) {
    panel.content.slice(0, 2).forEach(entry => {
      const val = document.createElement('span');
      val.textContent = entry.value;
      val.style.cssText = `margin-left:8px;font-size:11px;color:${STYLE_COLORS[entry.style] || STYLE_COLORS.normal}`;
      row.appendChild(val);
    });
  }

  container.appendChild(row);

  // Additional panels as separate rows
  data.panels.slice(1).forEach(panel => {
    const extraRow = document.createElement('div');
    extraRow.className = 'preview-dock-row';
    extraRow.innerHTML = `<span class="preview-dock-dot" style="color:${STATE_COLORS.healthy}">●</span>`;

    const label = document.createElement('span');
    label.textContent = panel.label;
    label.style.cssText = 'font-weight:700;color:#94a3b8;font-size:11px;letter-spacing:0.04em';
    extraRow.appendChild(label);

    if (panel.content) {
      panel.content.slice(0, 2).forEach(entry => {
        const val = document.createElement('span');
        val.textContent = entry.value;
        val.style.cssText = `margin-left:8px;font-size:11px;color:${STYLE_COLORS[entry.style] || STYLE_COLORS.normal}`;
        extraRow.appendChild(val);
      });
    }

    container.appendChild(extraRow);
  });
}

function renderTicker(data, container) {
  container.innerHTML = '';

  if (!data.metrics || data.metrics.length === 0) {
    container.innerHTML = '<span style="font-size:10px;color:#3d5070">No metrics</span>';
    return;
  }

  data.metrics.forEach((metric, i) => {
    if (i > 0) {
      const sep = document.createElement('span');
      sep.className = 'preview-pill-sep';
      sep.textContent = '│';
      container.appendChild(sep);
    }

    const pill = document.createElement('span');
    pill.className = 'preview-pill';

    const label = document.createElement('span');
    label.className = 'preview-pill-label';
    label.textContent = metric.label + ' ';
    pill.appendChild(label);

    const value = document.createElement('span');
    value.className = 'preview-pill-value';
    const displayVal = Number.isInteger(metric.value) ? metric.value.toString() : metric.value.toFixed(1);
    value.textContent = displayVal + (metric.unit || '');
    pill.appendChild(value);

    container.appendChild(pill);
  });
}

// ─── Scaffolder ───

const SCAFFOLDS = {
  python: (name, desc) => `"""${name}: dofek plugin — ${desc}"""

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

    processes = req.get("processes", [])

    resp = {
        "status": "ok",
        "panels": [
            {
                "id": "${name}-status",
                "label": "${name.toUpperCase()}",
                "content": [
                    {"key": "Status", "value": "running", "style": "accent"},
                ],
            }
        ],
        "process_annotations": [],
        "metrics": [],
    }

    if first:
        resp["manifest"] = {
            "name": "${name}",
            "version": "0.1.0",
            "description": "${desc}",
            "author": "",
        }
        first = False

    print(json.dumps(resp), flush=True)
`,

  rust: (name, desc) => `//! ${name}: dofek plugin — ${desc}

use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut first = true;

    for line in stdin.lock().lines().flatten() {
        if line.trim().is_empty() { continue; }

        let req: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        match req.get("type").and_then(|v| v.as_str()) {
            Some("shutdown") => break,
            Some("poll") => {}
            _ => continue,
        }

        let mut resp = serde_json::json!({
            "status": "ok",
            "panels": [{
                "id": "${name}-status",
                "label": "${name.toUpperCase()}",
                "content": [
                    {"key": "Status", "value": "running", "style": "accent"}
                ]
            }],
            "process_annotations": [],
            "metrics": []
        });

        if first {
            resp["manifest"] = serde_json::json!({
                "name": "${name}",
                "version": "0.1.0",
                "description": "${desc}",
                "author": ""
            });
            first = false;
        }

        let json = serde_json::to_string(&resp).unwrap();
        writeln!(stdout, "{json}").unwrap();
        stdout.flush().unwrap();
    }
}
`,

  node: (name, desc) => `#!/usr/bin/env node
// ${name}: dofek plugin — ${desc}

const readline = require('readline');

const rl = readline.createInterface({ input: process.stdin });
let first = true;

rl.on('line', (line) => {
  let req;
  try { req = JSON.parse(line.trim()); } catch { return; }

  if (req.type === 'shutdown') { process.exit(0); }
  if (req.type !== 'poll') { return; }

  const resp = {
    status: 'ok',
    panels: [{
      id: '${name}-status',
      label: '${name.toUpperCase()}',
      content: [
        { key: 'Status', value: 'running', style: 'accent' },
      ],
    }],
    process_annotations: [],
    metrics: [],
  };

  if (first) {
    resp.manifest = {
      name: '${name}',
      version: '0.1.0',
      description: '${desc}',
      author: '',
    };
    first = false;
  }

  process.stdout.write(JSON.stringify(resp) + '\\n');
});
`
};

function generateScaffold() {
  const name = document.getElementById('scaffold-name').value.trim() || 'my-plugin';
  const desc = document.getElementById('scaffold-desc').value.trim() || 'A dofek plugin';
  const lang = document.getElementById('scaffold-lang').value;

  const generator = SCAFFOLDS[lang];
  if (!generator) return;

  // Template literal interpolation within the scaffold strings
  let code = generator(name, desc);

  // Replace template markers (since we can't use real template literals inside strings)
  code = code.replaceAll('${name}', name);
  code = code.replaceAll('${name.toUpperCase()}', name.toUpperCase().replace(/-/g, ' '));
  code = code.replaceAll('${desc}', desc);

  const output = document.getElementById('scaffold-output');
  output.textContent = code;
}

function copyScaffold() {
  const output = document.getElementById('scaffold-output');
  const btn = document.getElementById('scaffolder-copy');

  navigator.clipboard.writeText(output.textContent).then(() => {
    btn.textContent = 'COPIED';
    btn.classList.add('copied');
    setTimeout(() => {
      btn.textContent = 'COPY';
      btn.classList.remove('copied');
    }, 2000);
  });
}

document.addEventListener('DOMContentLoaded', init);
