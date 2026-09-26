//! macOS GPU: temperature from the SMC, load and memory from IOAccelerator.
//!
//! Apple Silicon names none of its IOHID temperature sensors after the GPU, so
//! the component list (`components.rs`) finds no GPU there; the SMC's `Tg..`
//! keys are the GPU's (Intel Macs use `TG..`, plus `TCGC` for the integrated
//! one). IOAccelerator's `PerformanceStatistics` carries utilisation and the
//! memory the GPU holds. Neither needs root.

use std::ffi::c_void;
use std::mem::size_of;

use core_foundation::base::{CFType, TCFType, kCFAllocatorDefault};
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use io_kit_sys::types::io_connect_t;
use io_kit_sys::{
    IOConnectCallStructMethod, IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperty,
    IOServiceClose, IOServiceGetMatchingService, IOServiceGetMatchingServices, IOServiceMatching, IOServiceOpen,
    kIOMasterPortDefault,
};
use mach2::kern_return::KERN_SUCCESS;
use mach2::traps::mach_task_self;

use super::{Reading, average};
use crate::snapshot::{DeviceMetrics, Memory, round1};

/// `SMCKeyData_t` from Apple's SMC user client (80 bytes).
#[repr(C)]
#[derive(Default, Clone, Copy)]
struct KeyData {
    key: u32,
    vers: [u8; 6],
    p_limit: PLimitData,
    info: KeyInfo,
    result: u8,
    status: u8,
    data8: u8,
    data32: u32,
    bytes: [u8; 32],
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct PLimitData {
    version: u16,
    length: u16,
    cpu: u32,
    gpu: u32,
    mem: u32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct KeyInfo {
    data_size: u32,
    data_type: u32,
    data_attributes: u8,
}

const _: () = assert!(size_of::<KeyData>() == 80);

const SMC_HANDLE_EVENT: u32 = 2;
const SMC_READ_BYTES: u8 = 5;
const SMC_KEY_AT_INDEX: u8 = 8;
const SMC_KEY_INFO: u8 = 9;

const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}

struct Smc {
    conn: io_connect_t,
}

// The connection is a mach port, usable from any thread.
unsafe impl Send for Smc {}

impl Smc {
    fn open() -> Option<Self> {
        unsafe {
            let service = IOServiceGetMatchingService(kIOMasterPortDefault, IOServiceMatching(c"AppleSMC".as_ptr()));
            if service == 0 {
                return None;
            }
            let mut conn = 0;
            let kr = IOServiceOpen(service, mach_task_self(), 0, &mut conn);
            IOObjectRelease(service);
            (kr == KERN_SUCCESS).then_some(Self { conn })
        }
    }

    fn call(&self, input: KeyData) -> Option<KeyData> {
        let mut out = KeyData::default();
        let mut size = size_of::<KeyData>();
        let kr = unsafe {
            IOConnectCallStructMethod(
                self.conn,
                SMC_HANDLE_EVENT,
                (&input as *const KeyData).cast::<c_void>(),
                size_of::<KeyData>(),
                (&mut out as *mut KeyData).cast::<c_void>(),
                &mut size,
            )
        };
        (kr == KERN_SUCCESS && out.result == 0).then_some(out)
    }

    fn key_info(&self, key: u32) -> Option<KeyInfo> {
        self.call(KeyData { key, data8: SMC_KEY_INFO, ..Default::default() }).map(|o| o.info)
    }

    fn read(&self, key: u32, info: KeyInfo) -> Option<[u8; 32]> {
        self.call(KeyData { key, info, data8: SMC_READ_BYTES, ..Default::default() }).map(|o| o.bytes)
    }

    fn key_at(&self, index: u32) -> Option<u32> {
        self.call(KeyData { data32: index, data8: SMC_KEY_AT_INDEX, ..Default::default() }).map(|o| o.key)
    }

    fn key_count(&self) -> u32 {
        let key = fourcc(b"#KEY");
        self.key_info(key).and_then(|i| self.read(key, i)).map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// `flt ` on Apple Silicon, `sp78` (signed 8.8 fixed point) on Intel.
    fn temperature(&self, key: &SmcKey) -> Option<f64> {
        let b = self.read(key.code, key.info)?;
        let t = match &key.info.data_type.to_be_bytes() {
            b"flt " => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            b"sp78" => f64::from(i16::from_be_bytes([b[0], b[1]])) / 256.0,
            _ => return None,
        };
        (t > 0.0 && t < 150.0).then_some(t)
    }
}

impl Drop for Smc {
    fn drop(&mut self) {
        unsafe { IOServiceClose(self.conn) };
    }
}

struct SmcKey {
    code: u32,
    name: String,
    info: KeyInfo,
}

fn is_gpu_key(name: &[u8; 4]) -> bool {
    name.starts_with(b"Tg") || name.starts_with(b"TG") || name == b"TCGC"
}

/// Walk the SMC's key table once for the GPU temperature keys that read back.
fn gpu_keys(smc: &Smc) -> Vec<SmcKey> {
    (0..smc.key_count())
        .filter_map(|i| smc.key_at(i))
        .filter(|code| is_gpu_key(&code.to_be_bytes()))
        .filter_map(|code| {
            let info = smc.key_info(code)?;
            let key = SmcKey { code, name: String::from_utf8_lossy(&code.to_be_bytes()).into_owned(), info };
            smc.temperature(&key).is_some().then_some(key)
        })
        .collect()
}

/// What one IOAccelerator reports.
#[derive(Default)]
struct AccelStats {
    utilization: Option<f64>,
    used: Option<f64>,
    /// Only discrete GPUs have their own memory; Apple Silicon shares the RAM.
    total: Option<f64>,
}

fn number(dict: &CFDictionary, key: &str) -> Option<f64> {
    let key = CFString::new(key);
    let value = dict.find(key.as_CFTypeRef().cast())?;
    let value = unsafe { CFType::wrap_under_get_rule(*value) };
    value.downcast::<CFNumber>()?.to_f64()
}

fn accelerators() -> Vec<AccelStats> {
    let mut out = Vec::new();
    unsafe {
        let mut iter = 0;
        if IOServiceGetMatchingServices(kIOMasterPortDefault, IOServiceMatching(c"IOAccelerator".as_ptr()), &mut iter)
            != KERN_SUCCESS
        {
            return out;
        }
        let key = CFString::from_static_string("PerformanceStatistics");
        loop {
            let entry = IOIteratorNext(iter);
            if entry == 0 {
                break;
            }
            let prop = IORegistryEntryCreateCFProperty(entry, key.as_concrete_TypeRef(), kCFAllocatorDefault, 0);
            IOObjectRelease(entry);
            if prop.is_null() {
                continue;
            }
            let Some(stats) = CFType::wrap_under_create_rule(prop).downcast_into::<CFDictionary>() else { continue };
            let vram_used = number(&stats, "vramUsedBytes");
            out.push(AccelStats {
                utilization: number(&stats, "Device Utilization %").or_else(|| number(&stats, "GPU Activity(%)")),
                used: vram_used.or_else(|| number(&stats, "In use system memory")),
                total: vram_used.zip(number(&stats, "vramFreeBytes")).map(|(u, f)| u + f),
            });
        }
        IOObjectRelease(iter);
    }
    out
}

pub struct MacGpu {
    smc: Option<Smc>,
    keys: Vec<SmcKey>,
}

impl MacGpu {
    pub fn new() -> Self {
        let smc = Smc::open();
        let keys = smc.as_ref().map(gpu_keys).unwrap_or_default();
        Self { smc, keys }
    }

    fn temps(&self) -> Vec<(&SmcKey, f64)> {
        let Some(smc) = &self.smc else { return Vec::new() };
        self.keys.iter().filter_map(|k| Some((k, smc.temperature(k)?))).collect()
    }

    /// Fill in what the component list could not; `ram_total` stands in for
    /// the memory size of a GPU that shares the RAM.
    pub fn fill(&self, gpu: &mut DeviceMetrics, ram_total: u64) {
        let temps: Vec<f64> = self.temps().into_iter().map(|(_, t)| t).collect();
        if let Some(t) = average(&temps) {
            gpu.temp_c = Some(round1(t));
        }
        // The busiest one is the GPU in use (Intel Macs switch between two).
        let accels = accelerators();
        let Some(busy) = accels.iter().max_by(|a, b| a.utilization.unwrap_or(0.0).total_cmp(&b.utilization.unwrap_or(0.0)))
        else {
            return;
        };
        gpu.load_pct = busy.utilization.map(|u| round1(u.clamp(0.0, 100.0)));
        if let Some(used) = busy.used {
            gpu.mem = Memory::from_bytes(used as u64, busy.total.map_or(ram_total, |t| t as u64));
        }
    }

    pub fn readings(&self) -> Vec<Reading> {
        let mut out: Vec<Reading> = self
            .temps()
            .into_iter()
            .map(|(k, t)| Reading { source: "smc".into(), label: format!("GPU {}", k.name), value: t, unit: "°C" })
            .collect();
        for (i, a) in accelerators().iter().enumerate() {
            if let Some(u) = a.utilization {
                out.push(Reading { source: "ioaccelerator".into(), label: format!("GPU {i} utilization"), value: u, unit: "%" });
            }
            if let Some(used) = a.used {
                let mb = used / (1024.0 * 1024.0);
                out.push(Reading { source: "ioaccelerator".into(), label: format!("GPU {i} memory in use"), value: mb, unit: "MB" });
            }
        }
        out
    }
}
