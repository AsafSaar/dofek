//! dofek-docker: Plugin that queries the Docker Engine API and reports container status.
//!
//! Protocol: reads newline-delimited JSON from stdin, writes responses to stdout.
//! Communicates with Docker via its REST API (default http://localhost:2375).
//! On Windows, Docker Desktop exposes the API on tcp://localhost:2375 when enabled
//! in Settings > General > "Expose daemon on tcp://localhost:2375 without TLS".

use serde::{Deserialize, Serialize};
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
        if (args[i] == "--host" || args[i] == "--url") && i + 1 < args.len() {
            return args[i + 1].clone();
        }
        i += 1;
    }
    "http://localhost:2375".to_string()
}

fn handle_poll(host: &str, processes: &[ProcessContext], include_manifest: bool) -> PollResponse {
    let mut response = PollResponse {
        status: "ok".to_string(),
        manifest: if include_manifest {
            Some(Manifest {
                name: "dofek-docker".to_string(),
                version: "0.1.0".to_string(),
                description: "Docker container status and resource monitoring".to_string(),
                author: "dofek contributors".to_string(),
            })
        } else {
            None
        },
        panels: Vec::new(),
        process_annotations: Vec::new(),
        metrics: Vec::new(),
    };

    let containers = query_containers(host);

    let mut panel_content = Vec::new();

    match containers {
        Err(_) => {
            panel_content.push(PanelEntry {
                key: "Status".to_string(),
                value: "offline".to_string(),
                style: "dim".to_string(),
            });
        }
        Ok(containers) => {
            let running: Vec<_> = containers
                .iter()
                .filter(|c| c.state.as_deref() == Some("running"))
                .collect();

            if running.is_empty() {
                panel_content.push(PanelEntry {
                    key: "Status".to_string(),
                    value: "no containers".to_string(),
                    style: "dim".to_string(),
                });
            } else {
                // Show up to 3 container names
                for container in running.iter().take(3) {
                    let name = container
                        .names
                        .as_ref()
                        .and_then(|n| n.first())
                        .map(|n| n.trim_start_matches('/').to_string())
                        .unwrap_or_else(|| container.id[..12].to_string());

                    let image = container
                        .image
                        .as_deref()
                        .unwrap_or("unknown");

                    panel_content.push(PanelEntry {
                        key: name,
                        value: image.to_string(),
                        style: "accent".to_string(),
                    });
                }

                if running.len() > 3 {
                    panel_content.push(PanelEntry {
                        key: "...".to_string(),
                        value: format!("+{} more", running.len() - 3),
                        style: "dim".to_string(),
                    });
                }
            }

            // Metrics
            response.metrics.push(Metric {
                id: "docker.containers".to_string(),
                label: "Containers".to_string(),
                value: running.len() as f64,
                unit: String::new(),
            });

            // Annotate docker/containerd processes
            for proc in processes {
                let name_lower = proc.name.to_lowercase();
                if name_lower.contains("docker") || name_lower.contains("containerd") {
                    response.process_annotations.push(ProcessAnnotation {
                        pid: proc.pid,
                        label: Some(format!("{} containers", running.len())),
                        category: Some("dev".to_string()),
                        ai_state: None,
                    });
                }
            }
        }
    }

    response.panels.push(Panel {
        id: "docker-status".to_string(),
        label: "DOCKER".to_string(),
        content: panel_content,
    });

    response
}

// --- Docker API types ---

#[derive(Deserialize, Debug)]
struct DockerContainer {
    #[serde(rename = "Id")]
    id: String,
    #[serde(rename = "Names")]
    names: Option<Vec<String>>,
    #[serde(rename = "Image")]
    image: Option<String>,
    #[serde(rename = "State")]
    state: Option<String>,
}

fn query_containers(host: &str) -> Result<Vec<DockerContainer>, String> {
    let url = format!("{host}/containers/json?all=false");
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_millis(1000))
        .call()
        .map_err(|e| e.to_string())?;
    let body_str = resp.into_string().map_err(|e| e.to_string())?;
    let containers: Vec<DockerContainer> = serde_json::from_str(&body_str).map_err(|e| e.to_string())?;
    Ok(containers)
}

// --- Protocol types (mirror of dofek's plugin protocol) ---

#[derive(Deserialize, Debug)]
struct ProcessContext {
    pid: u32,
    name: String,
    #[allow(dead_code)]
    vram_bytes: Option<u64>,
}

#[derive(Serialize)]
struct PollResponse {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    manifest: Option<Manifest>,
    panels: Vec<Panel>,
    process_annotations: Vec<ProcessAnnotation>,
    metrics: Vec<Metric>,
}

#[derive(Serialize)]
struct Manifest {
    name: String,
    version: String,
    description: String,
    author: String,
}

#[derive(Serialize)]
struct Panel {
    id: String,
    label: String,
    content: Vec<PanelEntry>,
}

#[derive(Serialize)]
struct PanelEntry {
    key: String,
    value: String,
    style: String,
}

#[derive(Serialize)]
struct ProcessAnnotation {
    pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ai_state: Option<String>,
}

#[derive(Serialize)]
struct Metric {
    id: String,
    label: String,
    value: f64,
    #[serde(skip_serializing_if = "String::is_empty")]
    unit: String,
}
