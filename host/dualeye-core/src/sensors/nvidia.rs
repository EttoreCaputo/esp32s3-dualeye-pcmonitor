//! NVIDIA GPUs through NVML (`libnvidia-ml.so.1` / `nvml.dll`, installed with the driver).

use nvml_wrapper::Nvml;
use nvml_wrapper::enum_wrappers::device::{Clock, TemperatureSensor};

use super::Reading;
use crate::snapshot::{DeviceMetrics, round1};

pub struct Nvidia {
    nvml: Nvml,
}

impl Nvidia {
    pub fn init() -> Option<Self> {
        let nvml = Nvml::init().ok()?;
        (nvml.device_count().ok()? > 0).then_some(Self { nvml })
    }

    /// The first GPU's metrics and its fan speeds in RPM.
    pub fn sample(&self) -> Option<(DeviceMetrics, Vec<u32>)> {
        let dev = self.nvml.device_by_index(0).ok()?;
        let metrics = DeviceMetrics {
            temp_c: dev.temperature(TemperatureSensor::Gpu).ok().map(|t| t as f32),
            load_pct: dev.utilization_rates().ok().map(|u| u.gpu as f32),
            clock_mhz: dev.clock_info(Clock::Graphics).ok(),
            power_w: dev.power_usage().ok().map(|mw| round1(f64::from(mw) / 1000.0)),
        };
        let fans = (0..dev.num_fans().unwrap_or(0)).filter_map(|i| dev.fan_speed_rpm(i).ok()).collect();
        Some((metrics, fans))
    }

    pub fn readings(&self) -> Vec<Reading> {
        let mut out = Vec::new();
        for index in 0..self.nvml.device_count().unwrap_or(0) {
            let Ok(dev) = self.nvml.device_by_index(index) else { continue };
            let source = format!("nvml:{index} {}", dev.name().unwrap_or_default());
            let mut push = |label: String, value: f64, unit| {
                out.push(Reading { source: source.clone(), label, value, unit });
            };
            if let Ok(t) = dev.temperature(TemperatureSensor::Gpu) {
                push("GPU Temp".into(), f64::from(t), "°C");
            }
            if let Ok(u) = dev.utilization_rates() {
                push("GPU Load".into(), f64::from(u.gpu), "%");
            }
            if let Ok(mhz) = dev.clock_info(Clock::Graphics) {
                push("Graphics Clock".into(), f64::from(mhz), "MHz");
            }
            if let Ok(mw) = dev.power_usage() {
                push("Power".into(), f64::from(mw) / 1000.0, "W");
            }
            for fan in 0..dev.num_fans().unwrap_or(0) {
                if let Ok(rpm) = dev.fan_speed_rpm(fan) {
                    push(format!("fan{}", fan + 1), f64::from(rpm), "RPM");
                }
            }
        }
        out
    }
}
