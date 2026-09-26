//! Wire format shared with `main/metrics_parser.c`: one compact JSON object per line.

use serde::{Deserialize, Serialize};

use crate::claude::ClaudeMetrics;

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
    /// System RAM under `cpu`, VRAM under `gpu`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mem: Option<Memory>,
}

impl DeviceMetrics {
    pub fn is_empty(&self) -> bool {
        self.temp_c.is_none()
            && self.load_pct.is_none()
            && self.clock_mhz.is_none()
            && self.power_w.is_none()
            && self.mem.is_none()
    }
}

/// In MiB.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Memory {
    pub used_mb: u32,
    pub total_mb: u32,
}

impl Memory {
    pub fn from_bytes(used: u64, total: u64) -> Option<Self> {
        const MIB: u64 = 1024 * 1024;
        (total > 0).then(|| Self { used_mb: (used / MIB) as u32, total_mb: (total / MIB) as u32 })
    }
}

/// Watch face one round screen shows. Mirrors `metrics_face_t` in `main/metrics_model.h`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Face {
    /// Temperature, clock, power, load ring and fan.
    #[default]
    Classic,
    /// Three concentric rings: load, temperature, memory.
    Rings,
    /// Classic plus a RAM (left screen) or VRAM (right screen) bar.
    Plus,
    /// Classic plus a smaller memory bar, without the numbers.
    Bar,
    /// Claude Code: 5-hour and weekly limit rings, tokens, a small Clawd.
    Claude,
    /// Claude Code: a large animated Clawd showing whether Claude is working.
    Clawd,
}

impl Face {
    pub const ALL: [Face; 6] = [Face::Classic, Face::Rings, Face::Plus, Face::Bar, Face::Claude, Face::Clawd];

    pub fn name(self) -> &'static str {
        match self {
            Face::Classic => "classic",
            Face::Rings => "rings",
            Face::Plus => "plus",
            Face::Bar => "bar",
            Face::Claude => "claude",
            Face::Clawd => "clawd",
        }
    }
}

impl std::str::FromStr for Face {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Face::ALL.into_iter().find(|f| f.name() == s).ok_or_else(|| {
            let names: Vec<_> = Face::ALL.iter().map(|f| f.name()).collect();
            format!("unknown face `{s}`, expected one of: {}", names.join(", "))
        })
    }
}

/// Which face each screen shows: left (`cpu`) and right (`gpu`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Faces {
    #[serde(default, deserialize_with = "face_or_classic")]
    pub cpu: Face,
    #[serde(default, deserialize_with = "face_or_classic")]
    pub gpu: Face,
}

/// Names this build doesn't know (such as the retired `memory` and `gauge`)
/// read as `classic`, so an old settings file still loads.
fn face_or_classic<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Face, D::Error> {
    Ok(String::deserialize(d)?.parse().unwrap_or_default())
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
    /// Set by the bridge, not the collector. The firmware falls back to
    /// `classic` on both screens when it is missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub face: Option<Faces>,
    /// Claude Code usage, also set by the bridge; absent where Claude Code
    /// hasn't run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude: Option<ClaudeMetrics>,
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
                mem: Memory::from_bytes(12_884_901_888, 33_285_996_544),
            },
            gpu: DeviceMetrics {
                temp_c: Some(31.0),
                mem: Some(Memory { used_mb: 1024, total_mb: 24576 }),
                ..Default::default()
            },
            fans: vec![Fan { id: "cpu".into(), rpm: 3770 }],
            face: Some(Faces { cpu: Face::Rings, gpu: Face::Plus }),
            claude: None,
        };
        assert_eq!(
            snap.to_line(),
            "{\"v\":1,\"ts\":1700000000,\"cpu\":{\"temp_c\":36.3,\"load_pct\":1.8,\"clock_mhz\":1572,\"power_w\":14.6,\
             \"mem\":{\"used_mb\":12288,\"total_mb\":31744}},\"gpu\":{\"temp_c\":31.0,\"mem\":{\"used_mb\":1024,\"total_mb\":24576}},\
             \"fans\":[{\"id\":\"cpu\",\"rpm\":3770}],\"face\":{\"cpu\":\"rings\",\"gpu\":\"plus\"}}\n"
        );
    }

    #[test]
    fn empty_sections_are_omitted() {
        let snap = Snapshot {
            v: 1,
            ts: 0,
            cpu: DeviceMetrics::default(),
            gpu: DeviceMetrics::default(),
            fans: vec![],
            face: None,
            claude: None,
        };
        assert_eq!(snap.to_line(), "{\"v\":1,\"ts\":0}\n");
        assert!(!snap.is_sendable());
    }

    #[test]
    fn claude_section() {
        let snap = Snapshot {
            v: 1,
            ts: 0,
            cpu: DeviceMetrics { temp_c: Some(40.0), ..Default::default() },
            gpu: DeviceMetrics::default(),
            fans: vec![],
            face: Some(Faces { cpu: Face::Claude, gpu: Face::Clawd }),
            claude: Some(ClaudeMetrics {
                tok: 1_234_567,
                today: 4_500_000,
                left_min: Some(133),
                s_pct: Some(42.0),
                w_pct: None,
                state: crate::ClaudeState::Work,
                model: Some("OPUS 5.5".into()),
            }),
        };
        assert_eq!(
            snap.to_line(),
            "{\"v\":1,\"ts\":0,\"cpu\":{\"temp_c\":40.0},\"face\":{\"cpu\":\"claude\",\"gpu\":\"clawd\"},\
             \"claude\":{\"tok\":1234567,\"today\":4500000,\"left_min\":133,\"s_pct\":42.0,\"state\":\"work\",\"model\":\"OPUS 5.5\"}}\n"
        );
    }

    #[test]
    fn faces_parse_by_name() {
        assert_eq!("plus".parse::<Face>(), Ok(Face::Plus));
        assert_eq!("clawd".parse::<Face>(), Ok(Face::Clawd));
        assert!("gauge".parse::<Face>().is_err());
        let faces: Faces = serde_json::from_str("{\"gpu\":\"rings\"}").unwrap();
        assert_eq!(faces, Faces { cpu: Face::Classic, gpu: Face::Rings });
        let old: Faces = serde_json::from_str("{\"cpu\":\"gauge\",\"gpu\":\"memory\"}").unwrap();
        assert_eq!(old, Faces::default());
    }
}
