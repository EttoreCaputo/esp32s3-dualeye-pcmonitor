//! Wire format shared with `main/metrics_parser.c`: one compact JSON object per line.

use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DeviceMetrics {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temp_c: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_pct: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock_mhz: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub power_w: Option<f32>,
}

impl DeviceMetrics {
    pub fn is_empty(&self) -> bool {
        self.temp_c.is_none() && self.load_pct.is_none() && self.clock_mhz.is_none() && self.power_w.is_none()
    }
}

/// `id` is what the UI keys on: `"cpu"` (case/radiator fans) or `"gpu"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Fan {
    pub id: String,
    pub rpm: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub v: u32,
    pub ts: u64,
    #[serde(default, skip_serializing_if = "DeviceMetrics::is_empty")]
    pub cpu: DeviceMetrics,
    #[serde(default, skip_serializing_if = "DeviceMetrics::is_empty")]
    pub gpu: DeviceMetrics,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fans: Vec<Fan>,
}

impl Snapshot {
    /// The firmware drops lines that carry neither temperature.
    pub fn is_sendable(&self) -> bool {
        self.cpu.temp_c.is_some() || self.gpu.temp_c.is_some()
    }

    pub fn fan_rpm(&self, id: &str) -> Option<u32> {
        self.fans.iter().find(|f| f.id == id).map(|f| f.rpm)
    }

    pub fn to_line(&self) -> String {
        let mut line = serde_json::to_string(self).expect("snapshot is always serializable");
        line.push('\n');
        line
    }
}

pub(crate) fn round1(value: f64) -> f32 {
    ((value * 10.0).round() / 10.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_matches_firmware_format() {
        let snap = Snapshot {
            v: PROTOCOL_VERSION,
            ts: 1_700_000_000,
            cpu: DeviceMetrics {
                temp_c: Some(round1(36.33)),
                load_pct: Some(round1(1.81)),
                clock_mhz: Some(1572),
                power_w: Some(round1(14.596)),
            },
            gpu: DeviceMetrics { temp_c: Some(31.0), ..Default::default() },
            fans: vec![Fan { id: "cpu".into(), rpm: 3770 }],
        };
        assert_eq!(
            snap.to_line(),
            "{\"v\":1,\"ts\":1700000000,\"cpu\":{\"temp_c\":36.3,\"load_pct\":1.8,\"clock_mhz\":1572,\"power_w\":14.6},\
             \"gpu\":{\"temp_c\":31.0},\"fans\":[{\"id\":\"cpu\",\"rpm\":3770}]}\n"
        );
    }

    #[test]
    fn empty_sections_are_omitted() {
        let snap = Snapshot { v: 1, ts: 0, cpu: DeviceMetrics::default(), gpu: DeviceMetrics::default(), fans: vec![] };
        assert_eq!(snap.to_line(), "{\"v\":1,\"ts\":0}\n");
        assert!(!snap.is_sendable());
    }
}
