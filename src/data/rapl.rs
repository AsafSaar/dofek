//! Linux RAPL (Running Average Power Limit) CPU power reader.
//!
//! Reads `/sys/class/powercap/intel-rapl:0/energy_uj` — a monotonic microjoule
//! counter on supported Intel CPUs (and some AMD parts via `intel_rapl_common`).
//! On first read failure (permission denied, sysfs missing, AMD-without-driver,
//! …) the tracker self-disables so the hot loop stays quiet.
//!
//! The counter wraps at `max_energy_range_uj`; we read that once and use it to
//! recover the correct delta on overflow.

use std::time::Instant;

const ENERGY_PATH: &str = "/sys/class/powercap/intel-rapl:0/energy_uj";
const MAX_ENERGY_PATH: &str = "/sys/class/powercap/intel-rapl:0/max_energy_range_uj";

#[derive(Default)]
pub struct RaplTracker {
    prev_uj: Option<u64>,
    prev_at: Option<Instant>,
    max_uj: Option<u64>,
    disabled: bool,
}

impl RaplTracker {
    pub fn read_watts(&mut self) -> Option<f32> {
        if self.disabled {
            return None;
        }

        let uj = match read_u64(ENERGY_PATH) {
            Some(v) => v,
            None => {
                self.disabled = true;
                log::info!(
                    "RAPL CPU power unavailable (read {} failed) — disabling for this session",
                    ENERGY_PATH
                );
                return None;
            }
        };

        let now = Instant::now();

        if self.max_uj.is_none() {
            self.max_uj = read_u64(MAX_ENERGY_PATH);
        }

        let watts = match (self.prev_uj, self.prev_at) {
            (Some(prev), Some(at)) => {
                let elapsed = now.duration_since(at).as_secs_f64();
                if elapsed <= 0.0 {
                    None
                } else {
                    let delta_uj = if uj >= prev {
                        uj - prev
                    } else if let Some(max) = self.max_uj {
                        // Counter wrapped.
                        max.saturating_sub(prev).saturating_add(uj)
                    } else {
                        return {
                            self.prev_uj = Some(uj);
                            self.prev_at = Some(now);
                            None
                        };
                    };
                    Some((delta_uj as f64 / 1_000_000.0 / elapsed) as f32)
                }
            }
            _ => None,
        };

        self.prev_uj = Some(uj);
        self.prev_at = Some(now);
        watts
    }
}

fn read_u64(path: &str) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}
