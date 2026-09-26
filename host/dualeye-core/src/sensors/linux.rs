//! Linux: hwmon (`/sys/class/hwmon`) for temperatures, fans and AMD GPUs, and
//! RAPL (`/sys/class/powercap`) for CPU package power.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use super::{PlatformSample, Reading, average};
use crate::snapshot::{DeviceMetrics, round1};

const HWMON_ROOT: &str = "/sys/class/hwmon";
const POWERCAP_ROOT: &str = "/sys/class/powercap";

/// hwmon drivers whose temperatures are the CPU die/cores.
const CPU_DRIVERS: &[&str] = &["coretemp", "k10temp", "zenpower", "cpu_thermal", "cpu-thermal"];
/// hwmon drivers that belong to a graphics card; their fans are GPU fans.
const GPU_DRIVERS: &[&str] = &["amdgpu", "radeon", "nouveau", "i915", "xe"];

pub struct LinuxSensors {
    hwmon_root: PathBuf,
    rapl: Rapl,
}

impl LinuxSensors {
    pub fn new() -> Self {
        Self::with_roots(Path::new(HWMON_ROOT), Path::new(POWERCAP_ROOT))
    }

    fn with_roots(hwmon_root: &Path, powercap_root: &Path) -> Self {
        Self { hwmon_root: hwmon_root.to_path_buf(), rapl: Rapl::open(powercap_root) }
    }

    pub fn sample(&mut self) -> PlatformSample {
        let mut out = PlatformSample { cpu_power: self.rapl.power_w(), ..Default::default() };
        let mut cpu_temps = Vec::new();
        let mut amd_gpu: Option<(DeviceMetrics, Vec<u32>)> = None;

        for dev in hwmon_devices(&self.hwmon_root) {
            let fans: Vec<u32> = channels(&dev.dir, "fan").iter().map(|c| c.value.max(0.0) as u32).collect();
            if CPU_DRIVERS.contains(&dev.name.as_str()) {
                cpu_temps.extend(channels(&dev.dir, "temp").iter().map(|c| c.value / 1000.0));
            } else if dev.name == "amdgpu" || dev.name == "radeon" {
                // With an APU plus a discrete card, prefer the one that has fans.
                if amd_gpu.as_ref().is_none_or(|(_, f)| f.is_empty() && !fans.is_empty()) {
                    amd_gpu = Some((amd_gpu_metrics(&dev.dir), fans.clone()));
                }
                continue;
            }
            if GPU_DRIVERS.contains(&dev.name.as_str()) {
                out.gpu_fans.extend(fans);
            } else {
                out.board_fans.extend(fans);
            }
        }

        out.cpu_temp = average(&cpu_temps);
        if let Some((gpu, fans)) = amd_gpu {
            out.gpu = gpu;
            out.gpu_fans.extend(fans);
        }
        out
    }

    pub fn readings(&mut self) -> Vec<Reading> {
        let mut out = Vec::new();
        for dev in hwmon_devices(&self.hwmon_root) {
            let source = format!("{} ({})", dev.name, dev.dir.file_name().unwrap_or_default().to_string_lossy());
            for (kind, scale, unit) in [("temp", 1000.0, "°C"), ("fan", 1.0, "RPM"), ("power", 1e6, "W")] {
                for c in channels(&dev.dir, kind) {
                    out.push(Reading { source: source.clone(), label: c.label, value: c.value / scale, unit });
                }
            }
        }
        if let Some(watts) = self.rapl.power_w() {
            out.push(Reading { source: "rapl".into(), label: "CPU package".into(), value: watts, unit: "W" });
        }
        out
    }
}

struct HwmonDevice {
    name: String,
    dir: PathBuf,
}

fn hwmon_devices(root: &Path) -> Vec<HwmonDevice> {
    let Ok(entries) = fs::read_dir(root) else { return Vec::new() };
    let mut devices: Vec<HwmonDevice> = entries
        .flatten()
        .filter_map(|entry| {
            let dir = entry.path();
            // Old drivers keep their attributes under device/ instead of the hwmon node.
            [dir.clone(), dir.join("device")]
                .into_iter()
                .find_map(|d| Some(HwmonDevice { name: read_string(&d.join("name"))?, dir: d }))
        })
        .collect();
    devices.sort_by(|a, b| a.dir.cmp(&b.dir));
    devices
}

struct Channel {
    label: String,
    value: f64,
}

/// `<kind>N_input` values in raw hwmon units, with `<kind>N_label` when present.
/// Unreadable inputs (e.g. ENODEV on a disconnected header) are skipped.
fn channels(dir: &Path, kind: &str) -> Vec<Channel> {
    let Ok(entries) = fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<(u32, Channel)> = entries
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name();
            let index: u32 = file.to_str()?.strip_prefix(kind)?.strip_suffix("_input")?.parse().ok()?;
            let value = read_number(&entry.path())?;
            let label = read_string(&dir.join(format!("{kind}{index}_label"))).unwrap_or_else(|| format!("{kind}{index}"));
            Some((index, Channel { label, value }))
        })
        .collect();
    found.sort_by_key(|(index, _)| *index);
    found.into_iter().map(|(_, c)| c).collect()
}

fn amd_gpu_metrics(dir: &Path) -> DeviceMetrics {
    let temps = channels(dir, "temp");
    let temp = temps
        .iter()
        .find(|c| c.label == "edge")
        .or_else(|| temps.iter().find(|c| !matches!(c.label.as_str(), "junction" | "mem")))
        .map(|c| c.value / 1000.0);
    let power = read_number(&dir.join("power1_average")).or_else(|| read_number(&dir.join("power1_input")));
    DeviceMetrics {
        temp_c: temp.map(round1),
        load_pct: read_number(&dir.join("device/gpu_busy_percent")).map(|v| v as f32),
        clock_mhz: read_number(&dir.join("freq1_input")).map(|hz| (hz / 1e6).round() as u32),
        power_w: power.map(|uw| round1(uw / 1e6)),
    }
}

/// CPU package energy counters. `energy_uj` is root-only on most distros since
/// 2020; without read access power is simply left out.
struct Rapl {
    zones: Vec<RaplZone>,
}

struct RaplZone {
    energy: PathBuf,
    max_range: u64,
    last: Option<(u64, Instant)>,
}

impl Rapl {
    fn open(root: &Path) -> Self {
        let mut zones: Vec<RaplZone> = fs::read_dir(root)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|dir| {
                let name = dir.file_name().unwrap_or_default().to_string_lossy();
                // Top-level zones only: intel-rapl:0, not intel-rapl:0:0 (core/uncore subzones).
                name.starts_with("intel-rapl:") && name.matches(':').count() == 1
            })
            .filter(|dir| read_string(&dir.join("name")).is_some_and(|n| n.starts_with("package")))
            .filter(|dir| read_number(&dir.join("energy_uj")).is_some())
            .map(|dir| RaplZone {
                energy: dir.join("energy_uj"),
                max_range: read_number(&dir.join("max_energy_range_uj")).map_or(u64::MAX, |v| v as u64),
                last: None,
            })
            .collect();
        zones.sort_by(|a, b| a.energy.cmp(&b.energy));
        Self { zones }
    }

    /// Average package power since the previous call, summed over sockets.
    fn power_w(&mut self) -> Option<f64> {
        let mut total = None;
        for zone in &mut self.zones {
            let Some(now_uj) = read_number(&zone.energy).map(|v| v as u64) else { continue };
            let now = Instant::now();
            if let Some((prev_uj, prev)) = zone.last.replace((now_uj, now)) {
                let secs = now.duration_since(prev).as_secs_f64();
                if secs > 0.0 {
                    let delta = if now_uj >= prev_uj { now_uj - prev_uj } else { zone.max_range - prev_uj + now_uj };
                    *total.get_or_insert(0.0) += delta as f64 / 1e6 / secs;
                }
            }
        }
        total
    }
}

fn read_string(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

fn read_number(path: &Path) -> Option<f64> {
    read_string(path)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, rel: &str, value: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("{value}\n")).unwrap();
    }

    /// Layout taken from an Arrow Lake desktop (coretemp + nct6799) with an AMD card added.
    fn fake_sysfs() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let hw = tmp.path().join("hwmon");
        write(&hw, "hwmon0/name", "acpi_fan");
        write(&hw, "hwmon1/name", "coretemp");
        write(&hw, "hwmon1/temp1_input", "37000");
        write(&hw, "hwmon1/temp1_label", "Package id 0");
        write(&hw, "hwmon1/temp6_input", "38000");
        write(&hw, "hwmon1/temp38_input", "34000");
        write(&hw, "hwmon2/name", "nct6799");
        write(&hw, "hwmon2/fan1_input", "791");
        write(&hw, "hwmon2/fan2_input", "508");
        write(&hw, "hwmon2/fan7_input", "3770");
        write(&hw, "hwmon2/temp1_input", "30000");
        write(&hw, "hwmon3/name", "amdgpu");
        write(&hw, "hwmon3/temp1_input", "31000");
        write(&hw, "hwmon3/temp1_label", "edge");
        write(&hw, "hwmon3/temp2_input", "41000");
        write(&hw, "hwmon3/temp2_label", "junction");
        write(&hw, "hwmon3/fan1_input", "0");
        write(&hw, "hwmon3/power1_average", "20000000");
        write(&hw, "hwmon3/freq1_input", "210000000");
        write(&hw, "hwmon3/device/gpu_busy_percent", "3");
        let pc = tmp.path().join("powercap");
        write(&pc, "intel-rapl:0/name", "package-0");
        write(&pc, "intel-rapl:0/energy_uj", "1000000");
        write(&pc, "intel-rapl:0/max_energy_range_uj", "262143328850");
        write(&pc, "intel-rapl:0:0/name", "core");
        write(&pc, "intel-rapl:0:0/energy_uj", "5");
        tmp
    }

    #[test]
    fn maps_hwmon_to_snapshot_fields() {
        let tmp = fake_sysfs();
        let mut sensors = LinuxSensors::with_roots(&tmp.path().join("hwmon"), &tmp.path().join("powercap"));
        let s = sensors.sample();
        assert!((s.cpu_temp.unwrap() - 36.333).abs() < 0.01);
        assert_eq!(s.board_fans, vec![791, 508, 3770]);
        assert_eq!(s.gpu_fans, vec![0]);
        assert_eq!(
            s.gpu,
            DeviceMetrics { temp_c: Some(31.0), load_pct: Some(3.0), clock_mhz: Some(210), power_w: Some(20.0) }
        );
        // First RAPL read only primes the counter.
        assert_eq!(s.cpu_power, None);
        assert_eq!(sensors.rapl.zones.len(), 1);
    }

    #[test]
    fn rapl_handles_counter_wrap() {
        let tmp = fake_sysfs();
        let pc = tmp.path().join("powercap");
        let mut rapl = Rapl::open(&pc);
        write(&pc, "intel-rapl:0/energy_uj", "262143328000");
        rapl.power_w();
        write(&pc, "intel-rapl:0/energy_uj", "150");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let watts = rapl.power_w().unwrap();
        assert!(watts > 0.0 && watts < 1.0, "{watts}");
    }
}
