//! dofek-ollama: Plugin that queries the Ollama API and reports model status.
//!
//! Protocol: reads newline-delimited JSON from stdin, writes responses to stdout.
//! Communicates with Ollama via its REST API (default http://localhost:11434).

use dofek_plugin_protocol::{
    Metric, Panel, PanelEntry, PluginManifest, PollResponse, ProcessAnnotation, ProcessContext,
};
use serde::Deserialize;
use std::io::{self, BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let host = parse_host(&args);

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    let mut first_response = true;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let msg_type = request.get("type").and_then(|v| v.as_str()).unwrap_or("");

        match msg_type {
            "shutdown" => break,
            "poll" => {
                let processes: Vec<ProcessContext> = request
                    .get("processes")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                    .unwrap_or_default();

                let response = handle_poll(&host, &processes, first_response);
                first_response = false;

                let json = serde_json::to_string(&response).unwrap_or_default();
                let _ = writeln!(stdout, "{json}");
                let _ = stdout.flush();
            }
            _ => {}
        }
    }
}

fn parse_host(args: &[String]) -> String {
    let mut i = 1;
    while i < args.len() {
        if (args[i] == "--host" || args[i] == "--port") && i + 1 < args.len() {
            return args[i + 1].clone();
        }
        i += 1;
    }
    "http://localhost:11434".to_string()
}

fn handle_poll(host: &str, processes: &[ProcessContext], include_manifest: bool) -> PollResponse {
    let mut response = PollResponse {
        status: "ok".to_string(),
        manifest: if include_manifest {
            Some(PluginManifest {
                name: "dofek-ollama".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: "Ollama model status and inference tracking".to_string(),
                author: "Dofek contributors".to_string(),
            })
        } else {
            None
        },
        panels: Vec::new(),
        process_annotations: Vec::new(),
        metrics: Vec::new(),
    };

    let running = query_running_models(host);
    let available = query_available_models(host);

    let mut panel_content = Vec::new();

    match (&running, &available) {
        (Err(_), _) => {
            panel_content.push(PanelEntry {
                key: "Status".to_string(),
                value: "offline".to_string(),
                style: "dim".to_string(),
            });
        }
        (Ok(running_models), Ok(available_models)) => {
            if running_models.is_empty() {
                panel_content.push(PanelEntry {
                    key: "Status".to_string(),
                    value: "idle".to_string(),
                    style: "dim".to_string(),
                });
            } else {
                for model in running_models {
                    let size_str = format_size(model.size);
                    panel_content.push(PanelEntry {
                        key: "Model".to_string(),
                        value: format!("{} ({})", model.name, size_str),
                        style: "accent".to_string(),
                    });
                }
            }

            if !available_models.is_empty() {
                panel_content.push(PanelEntry {
                    key: "Available".to_string(),
                    value: format!("{} models", available_models.len()),
                    style: "dim".to_string(),
                });
            }

            response.metrics.push(Metric {
                id: "ollama.running".to_string(),
                label: "Models".to_string(),
                value: running_models.len() as f64,
                unit: String::new(),
            });

            for proc in processes {
                let name_lower = proc.name.to_lowercase();
                if name_lower.contains("ollama") {
                    let label = if !running_models.is_empty() {
                        running_models
                            .iter()
                            .map(|m| m.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    } else {
                        "idle".to_string()
                    };

                    let ai_state = if running_models.iter().any(|m| m.size > 0) {
                        Some("idle".to_string())
                    } else {
                        None
                    };

                    response.process_annotations.push(ProcessAnnotation {
                        pid: proc.pid,
                        label: Some(label),
                        category: Some("ai".to_string()),
                        ai_state,
                    });
                }
            }
        }
        (Ok(_), Err(_)) => {
            panel_content.push(PanelEntry {
                key: "Status".to_string(),
                value: "connected".to_string(),
                style: "normal".to_string(),
            });
        }
    }

    response.panels.push(Panel {
        id: "ollama-status".to_string(),
        label: "OLLAMA".to_string(),
        content: panel_content,
    });

    response
}

// --- Ollama API types ---

#[derive(Deserialize, Debug)]
struct OllamaTagsResponse {
    models: Option<Vec<OllamaModel>>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct OllamaModel {
    name: String,
    size: u64,
}

#[derive(Deserialize, Debug)]
struct OllamaPsResponse {
    models: Option<Vec<OllamaRunningModel>>,
}

#[derive(Deserialize, Debug)]
struct OllamaRunningModel {
    name: String,
    size: u64,
}

fn query_available_models(host: &str) -> Result<Vec<OllamaModel>, String> {
    let url = format!("{host}/api/tags");
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_millis(1000))
        .call()
        .map_err(|e| e.to_string())?;
    let body_str = resp.into_string().map_err(|e| e.to_string())?;
    let body: OllamaTagsResponse = serde_json::from_str(&body_str).map_err(|e| e.to_string())?;
    Ok(body.models.unwrap_or_default())
}

fn query_running_models(host: &str) -> Result<Vec<OllamaRunningModel>, String> {
    let url = format!("{host}/api/ps");
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_millis(1000))
        .call()
        .map_err(|e| e.to_string())?;
    let body_str = resp.into_string().map_err(|e| e.to_string())?;
    let body: OllamaPsResponse = serde_json::from_str(&body_str).map_err(|e| e.to_string())?;
    Ok(body.models.unwrap_or_default())
}

fn format_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1}GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.0}MB", bytes as f64 / 1_048_576.0)
    } else {
        format!("{bytes}B")
    }
}
