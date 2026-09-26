//! USB serial link to the board (ESP32-S3 USB Serial/JTAG, 303a:1001).

use std::io;
use std::time::Duration;

use serde::Serialize;
use serialport::{SerialPort, SerialPortType};

pub const ESPRESSIF_VID: u16 = 0x303A;
pub const BAUD_RATE: u32 = 115_200;

#[derive(Debug, Clone, Serialize)]
pub struct PortInfo {
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    pub product: Option<String>,
    /// USB vendor is Espressif.
    pub is_board: bool,
}

/// USB serial ports, sorted by name.
pub fn list_ports() -> Vec<PortInfo> {
    let mut ports: Vec<PortInfo> = serialport::available_ports()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| match p.port_type {
            SerialPortType::UsbPort(usb) => Some(PortInfo {
                name: p.port_name,
                vid: usb.vid,
                pid: usb.pid,
                product: usb.product,
                is_board: usb.vid == ESPRESSIF_VID,
            }),
            // Legacy UARTs (/dev/ttyS*, COM1) and Bluetooth can never be the board.
            _ => None,
        })
        .collect();
    // macOS lists every USB device twice (/dev/tty.* and /dev/cu.*); cu.* is the one to open.
    ports.retain(|p| !p.name.starts_with("/dev/tty."));
    ports.sort_by(|a, b| a.name.cmp(&b.name));
    ports
}

/// The board's port when exactly one Espressif device is attached.
pub fn detect_board() -> Option<String> {
    let mut boards = list_ports().into_iter().filter(|p| p.is_board);
    match (boards.next(), boards.next()) {
        (Some(port), None) => Some(port.name),
        _ => None,
    }
}

/// Open without toggling DTR/RTS: on the S3's USB Serial/JTAG that sequence
/// resets the chip (or drops it into the ROM bootloader).
pub fn open(name: &str) -> io::Result<Box<dyn SerialPort>> {
    let mut port = serialport::new(name, BAUD_RATE)
        .timeout(Duration::from_millis(200))
        .dtr_on_open(false)
        .open()?;
    port.write_data_terminal_ready(false)?;
    port.write_request_to_send(false)?;
    Ok(port)
}
