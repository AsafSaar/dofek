use anyhow::{Context, Result};
use serde::Deserialize;
use std::io::Read;

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

/// Upper bound on a `data.json` body. A real LHM tree is tens of kilobytes;
/// this is only here so a wrong or hostile `lhm.url` can't stream unbounded
/// data into memory on every poll.
const MAX_LHM_BODY_BYTES: u64 = 8 * 1024 * 1024;

/// Fetch and parse the LHM data.json tree.
pub fn fetch_lhm_data(base_url: &str) -> Result<LhmNode> {
    let url = format!("{}/data.json", base_url.trim_end_matches('/'));
    let response = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(2))
        .call()
        .with_context(|| format!("Failed to connect to LHM at {url}"))?;
    // `into_string()` reads to EOF with no limit. Cap it instead: a truncated
    // body fails to parse as JSON, which is the correct outcome.
    let mut body = String::new();
    response
        .into_reader()
        .take(MAX_LHM_BODY_BYTES)
        .read_to_string(&mut body)
        .context("Failed to read LHM response body")?;
    if body.len() as u64 >= MAX_LHM_BODY_BYTES {
        anyhow::bail!("LHM response exceeded {MAX_LHM_BODY_BYTES} bytes — refusing to parse");
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_values_with_units() {
        assert_eq!(parse_lhm_value("64.3 %"), Some(64.3));
        assert_eq!(parse_lhm_value("1200 MHz"), Some(1200.0));
        assert_eq!(parse_lhm_value("45.0 °C"), Some(45.0));
        assert_eq!(parse_lhm_value("12 W"), Some(12.0));
        assert_eq!(parse_lhm_value("0.5"), Some(0.5));
        assert_eq!(parse_lhm_value("  7.25 GB  "), Some(7.25));
        assert_eq!(parse_lhm_value("-5 °C"), Some(-5.0));
    }

    /// Some LHM locales emit a comma decimal separator.
    #[test]
    fn parses_comma_decimal_separator() {
        assert_eq!(parse_lhm_value("64,3 %"), Some(64.3));
        assert_eq!(parse_lhm_value("1,5 GB"), Some(1.5));
    }

    #[test]
    fn rejects_unparseable_values() {
        // LHM's "no reading" sentinel, and assorted junk.
        assert_eq!(parse_lhm_value("-"), None);
        assert_eq!(parse_lhm_value(""), None);
        assert_eq!(parse_lhm_value("   "), None);
        assert_eq!(parse_lhm_value("N/A"), None);
        assert_eq!(parse_lhm_value("MHz"), None);
        // Leading unit means nothing numeric gets taken.
        assert_eq!(parse_lhm_value("%64.3"), None);
        // Malformed numerics must not panic on the way to None.
        assert_eq!(parse_lhm_value("1.2.3"), None);
        assert_eq!(parse_lhm_value("--"), None);
        assert_eq!(parse_lhm_value("."), None);
    }

    fn node(text: &str, value: Option<&str>, children: Vec<LhmNode>) -> LhmNode {
        LhmNode {
            id: 0,
            text: text.to_string(),
            children,
            min: None,
            max: None,
            value: value.map(str::to_string),
            image_url: None,
        }
    }

    fn sample_tree() -> LhmNode {
        node(
            "Sensor",
            None,
            vec![node(
                "MYPC",
                None,
                vec![node(
                    "Intel Core i9-13900K",
                    None,
                    vec![
                        node(
                            "Load",
                            None,
                            vec![
                                node("CPU Total", Some("42.5 %"), vec![]),
                                node("CPU Core #1", Some("31.0 %"), vec![]),
                            ],
                        ),
                        node("Temperatures", None, vec![node("Core Max", Some("71 °C"), vec![])]),
                    ],
                )],
            )],
        )
    }

    #[test]
    fn find_path_walks_the_tree_case_insensitively() {
        let root = sample_tree();

        let total = root.find_path(&["MYPC", "Intel", "Load", "CPU Total"]).unwrap();
        assert_eq!(total.value_f32(), Some(42.5));

        // Matching is a case-insensitive substring, not an exact compare.
        let same = root.find_path(&["mypc", "intel core", "load", "cpu total"]).unwrap();
        assert_eq!(same.text, total.text);

        // An empty path is the node itself.
        assert_eq!(root.find_path(&[]).unwrap().text, "Sensor");
    }

    #[test]
    fn find_path_returns_none_for_a_missing_segment() {
        let root = sample_tree();
        assert!(root.find_path(&["MYPC", "AMD"]).is_none());
        assert!(root.find_path(&["MYPC", "Intel", "Load", "GPU Total"]).is_none());
        // A missing segment mid-path fails the whole walk.
        assert!(root.find_path(&["MYPC", "nope", "Load"]).is_none());
    }

    #[test]
    fn extracts_cpu_sensors_from_a_realistic_tree() {
        let cpu = extract_cpu(&sample_tree()).expect("CPU node should be found");
        assert_eq!(cpu.name, "Intel Core i9-13900K");
        assert_eq!(cpu.total_load, 42.5);
        assert_eq!(cpu.per_core_load, vec![31.0]);
        assert_eq!(cpu.temperature, Some(71.0));
        assert_eq!(cpu.power, None);
    }

    /// `LhmNode` is recursive, so a hostile or corrupt `data.json` could try to
    /// blow the stack during deserialization. serde_json enforces a depth limit,
    /// which must surface as an `Err` rather than a crash — dofek fetches this
    /// from a network endpoint whose URL is user-configurable.
    #[test]
    fn deeply_nested_json_errors_instead_of_overflowing_the_stack() {
        const DEPTH: usize = 10_000;
        let mut json = String::with_capacity(DEPTH * 40);
        for _ in 0..DEPTH {
            json.push_str(r#"{"id":0,"Text":"x","Children":["#);
        }
        json.push_str(r#"{"id":0,"Text":"leaf"}"#);
        for _ in 0..DEPTH {
            json.push_str("]}");
        }

        let parsed = serde_json::from_str::<LhmNode>(&json);
        assert!(parsed.is_err(), "10k-deep JSON should be rejected, not parsed");
    }

    #[test]
    fn missing_optional_fields_deserialize() {
        // Only `id` and `Text` are required; everything else defaults.
        let n: LhmNode = serde_json::from_str(r#"{"id":7,"Text":"bare"}"#).unwrap();
        assert_eq!(n.id, 7);
        assert!(n.children.is_empty());
        assert_eq!(n.value_f32(), None);
    }

    #[test]
    fn extract_helpers_tolerate_an_empty_tree() {
        let empty = node("Sensor", None, vec![]);
        assert!(extract_cpu(&empty).is_none());
        assert!(extract_memory(&empty).is_none());
        assert!(extract_gpus(&empty).is_empty());
    }
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
