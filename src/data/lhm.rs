use anyhow::{Context, Result};
use serde::Deserialize;

/// Node in the LibreHardwareMonitor JSON tree.
#[derive(Deserialize, Debug, Clone)]
pub struct LhmNode {
    pub id: i64,
    #[serde(rename = "Text")]
    pub text: String,
    #[serde(rename = "Children", default)]
    pub children: Vec<LhmNode>,
    #[serde(rename = "Min", default)]
    pub min: Option<String>,
    #[serde(rename = "Max", default)]
    pub max: Option<String>,
    #[serde(rename = "Value", default)]
    pub value: Option<String>,
    #[serde(rename = "ImageURL", default)]
    pub image_url: Option<String>,
}

impl LhmNode {
    /// Find a child node by text (case-insensitive substring match).
    pub fn find_child(&self, text: &str) -> Option<&LhmNode> {
        let text_lower = text.to_lowercase();
        self.children.iter().find(|c| c.text.to_lowercase().contains(&text_lower))
    }

    /// Find all children matching a text pattern.
    pub fn find_children(&self, text: &str) -> Vec<&LhmNode> {
        let text_lower = text.to_lowercase();
        self.children.iter().filter(|c| c.text.to_lowercase().contains(&text_lower)).collect()
    }

    /// Recursively find a node by walking a path of text patterns.
    pub fn find_path(&self, path: &[&str]) -> Option<&LhmNode> {
        if path.is_empty() {
            return Some(self);
        }
        let child = self.find_child(path[0])?;
        if path.len() == 1 {
            Some(child)
        } else {
            child.find_path(&path[1..])
        }
    }

    /// Parse the Value field as a float, stripping units like "%" , "MHz", "W", etc.
    pub fn value_f32(&self) -> Option<f32> {
        parse_lhm_value(self.value.as_deref()?)
    }
}

/// Parse LHM value strings like "64.3 %" or "1200 MHz" or "45.0 °C" into f32.
pub fn parse_lhm_value(s: &str) -> Option<f32> {
    let s = s.trim();
    if s == "-" || s.is_empty() {
        return None;
    }
    // Take everything up to the first non-numeric character (after optional leading minus and digits/dots)
    let numeric: String = s.chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == ',')
        .collect();
    // Handle comma as decimal separator
    let numeric = numeric.replace(',', ".");
    numeric.parse().ok()
}

/// Fetch and parse the LHM data.json tree.
pub fn fetch_lhm_data(base_url: &str) -> Result<LhmNode> {
    let url = format!("{}/data.json", base_url.trim_end_matches('/'));
    let response = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(2))
        .call()
        .with_context(|| format!("Failed to connect to LHM at {url}"))?;
    let body = response.into_string()
        .context("Failed to read LHM response body")?;
    let node: LhmNode = serde_json::from_str(&body)
        .context("Failed to parse LHM JSON response")?;
    Ok(node)
}

/// Extract CPU data from an LHM tree.
pub fn extract_cpu(root: &LhmNode) -> Option<CpuSensors> {
    // Navigate: root -> Computer -> CPU hardware node
    let computer = root.children.first()?;

    // Find the CPU node (contains "Intel" or "AMD" or has CPU-like sensors)
    let cpu_node = computer.children.iter().find(|n| {
        let t = n.text.to_lowercase();
        t.contains("intel") || t.contains("amd") || t.contains("cpu") || t.contains("processor")
    })?;

    let name = cpu_node.text.clone();

    // Find Load section
    let load_section = cpu_node.find_child("load")?;
    let total_load = load_section.find_child("cpu total").and_then(|n| n.value_f32()).unwrap_or(0.0);
    let per_core_load: Vec<f32> = load_section.children.iter()
        .filter(|n| {
            let t = n.text.to_lowercase();
            t.contains("cpu core #") || t.contains("core #")
        })
        .filter(|n| !n.text.to_lowercase().contains("total"))
        .filter_map(|n| n.value_f32())
        .collect();

    // Temperature
    let temperature = cpu_node.find_child("temperature")
        .and_then(|t| t.find_child("core").or_else(|| t.find_child("package")).or_else(|| t.find_child("cpu")))
        .and_then(|n| n.value_f32());

    // Power
    let power = cpu_node.find_child("power")
        .and_then(|p| p.find_child("package").or_else(|| p.find_child("cpu")))
        .and_then(|n| n.value_f32());

    Some(CpuSensors {
        name,
        total_load,
        per_core_load,
        temperature,
        power,
    })
}

/// Extract memory data from an LHM tree.
pub fn extract_memory(root: &LhmNode) -> Option<MemorySensors> {
    let computer = root.children.first()?;
    let mem_node = computer.children.iter().find(|n| {
        n.text.to_lowercase().contains("memory")
    })?;

    let load_section = mem_node.find_child("load");
    let data_section = mem_node.find_child("data");

    let used_percent = load_section
        .and_then(|l| l.find_child("memory").and_then(|n| n.value_f32()))
        .unwrap_or(0.0);

    let used_gb = data_section.as_ref()
        .and_then(|d| d.find_child("used memory").and_then(|n| n.value_f32()))
        .unwrap_or(0.0);

    let available_gb = data_section.as_ref()
        .and_then(|d| d.find_child("available memory").and_then(|n| n.value_f32()))
        .unwrap_or(0.0);

    let total_gb = used_gb + available_gb;

    let swap_used_percent = load_section
        .and_then(|l| l.find_child("virtual memory").and_then(|n| n.value_f32()))
        .unwrap_or(0.0);

    Some(MemorySensors {
        used_percent,
        used_gb,
        total_gb,
        swap_used_percent,
    })
}

/// Extract GPU data from an LHM tree (all GPU devices found).
pub fn extract_gpus(root: &LhmNode) -> Vec<GpuSensors> {
    let Some(computer) = root.children.first() else {
        return Vec::new();
    };

    computer.children.iter()
        .filter(|n| {
            let t = n.text.to_lowercase();
            t.contains("nvidia") || t.contains("radeon") || t.contains("geforce") || t.contains("gpu")
        })
        .filter_map(extract_single_gpu)
        .collect()
}

fn extract_single_gpu(gpu_node: &LhmNode) -> Option<GpuSensors> {
    let name = gpu_node.text.clone();

    let load_section = gpu_node.find_child("load");
    let utilization = load_section
        .and_then(|l| l.find_child("gpu core").and_then(|n| n.value_f32()))
        .unwrap_or(0.0);

    let temp_section = gpu_node.find_child("temperature");
    let temperature = temp_section
        .and_then(|t| t.find_child("gpu core").or_else(|| t.children.first()))
        .and_then(|n| n.value_f32())
        .unwrap_or(0.0);

    let power_section = gpu_node.find_child("power");
    let power_watts = power_section
        .and_then(|p| p.find_child("gpu").or_else(|| p.children.first()))
        .and_then(|n| n.value_f32())
        .unwrap_or(0.0);

    // VRAM from LHM (small bar data section)
    let data_section = gpu_node.find_child("data").or_else(|| gpu_node.find_child("small data"));
    let vram_used_mb = data_section.as_ref()
        .and_then(|d| d.find_child("gpu memory used").and_then(|n| n.value_f32()))
        .unwrap_or(0.0);
    let vram_total_mb = data_section.as_ref()
        .and_then(|d| d.find_child("gpu memory total").and_then(|n| n.value_f32()))
        .unwrap_or(0.0);

    // If total is reported in GB, convert to MB
    let (vram_used_mb, vram_total_mb) = if vram_total_mb < 100.0 {
        (vram_used_mb * 1024.0, vram_total_mb * 1024.0)
    } else {
        (vram_used_mb, vram_total_mb)
    };

    Some(GpuSensors {
        name,
        utilization,
        vram_used_mb,
        vram_total_mb,
        temperature,
        power_watts,
    })
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct CpuSensors {
    pub name: String,
    pub total_load: f32,
    pub per_core_load: Vec<f32>,
    pub temperature: Option<f32>,
    pub power: Option<f32>,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct MemorySensors {
    pub used_percent: f32,
    pub used_gb: f32,
    pub total_gb: f32,
    pub swap_used_percent: f32,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct GpuSensors {
    pub name: String,
    pub utilization: f32,
    pub vram_used_mb: f32,
    pub vram_total_mb: f32,
    pub temperature: f32,
    pub power_watts: f32,
}
