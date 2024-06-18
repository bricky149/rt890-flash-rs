/*
    Copyright 2024 Bricky
    https://github.com/bricky149

    Licensed under the Apache License, Version 2.0 (the "License");
    you may not use this file except in compliance with the License.
    You may obtain a copy of the License at

        http://www.apache.org/licenses/LICENSE-2.0

    Unless required by applicable law or agreed to in writing, software
    distributed under the License is distributed on an "AS IS" BASIS,
    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
    See the License for the specific language governing permissions and
    limitations under the License.
*/

extern crate serialport5;
use self::serialport5::*;

use crate::{fileops, uart};

use std::io::Write;
use std::time::Duration;

pub const FIRMWARE_SIZE: usize = 60_416;
pub const SPI_FLASH_SIZE: usize = 4_194_304;

const BAUD_RATE: u32 = 115_200;
const CHUNK_LENGTH: usize = 128;

pub struct SpiRange {
    pub cmd: u8,
    pub offset: usize,
    size: usize
}

pub fn dump_spi_flash(port: &String, filepath: &String) {
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_secs(20)))
        .open(port)
        .expect("Failed to open port. Are you running with root/admin privileges?");

    let mut fw = match fileops::create_file(filepath) {
        Some(f) => f,
        _ => return         // Panic already called from function
    };

    for offset in 0..32768 {
        match uart::command_readspiflash(&port, offset) {
            Ok(Some(data)) => {
                print!("\rDumping SPI flash from address {:#06x}", offset);
                fw.write_all(&data).expect("Failed to dump SPI flash")
            }
            Ok(None) => break,
            Err(e) => panic!("{}. Ensure the radio is in normal mode.", e)
        }
    }
}

pub fn restore_spi_flash(port: &String, calib_only: bool, filepath: &String) -> Result<bool> {
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_secs(20)))
        .open(port)
        .expect("Failed to open port. Are you running with root/admin privileges?");

    let spi = match fileops::read_file(filepath, SPI_FLASH_SIZE) {
        Some(f) => f,
        _ => return Ok(false)   // Either None was returned or a panic was called
    };

    // TODO: Document these magic command bytes
    let spi_ranges;
    if calib_only {
        spi_ranges = vec![
            SpiRange { cmd: 0x48, offset: 3928064, size: 4096 }     // 3BF000 Calibration data
        ];
    } else {
        spi_ranges = vec![
            SpiRange { cmd: 0x40, offset: 0, size: 2949120 },
            SpiRange { cmd: 0x41, offset: 2949120, size: 163840 },
            SpiRange { cmd: 0x42, offset: 3112960, size: 139264 },
            SpiRange { cmd: 0x43, offset: 3252224, size: 8192 },
            SpiRange { cmd: 0x47, offset: 3887104, size: 40960 },
            SpiRange { cmd: 0x48, offset: 3928064, size: 4096 },    // 3BF000 Calibration data
            SpiRange { cmd: 0x49, offset: 3936256, size: 40960 },
            SpiRange { cmd: 0x4b, offset: 4030464, size: 40960 },
            SpiRange { cmd: 0x4c, offset: 3260416, size: 626688 }
        ]; 
    }

    for spi_range in spi_ranges {
        let mut offset = spi_range.offset;
        let block_length = offset + spi_range.size;

        while offset < block_length {
            match uart::command_writespiflash(&port, &spi_range, offset, &spi) {
                Ok(true) => print!("\rRestoring SPI flash to address {:#08x}", offset),
                _ => panic!("Failed to restore SPI flash. Ensure the radio is in normal mode.")
            }
            offset += CHUNK_LENGTH
        }
    }

    Ok(true)
}

pub fn flash_firmware(port: &String, filepath: &String) -> Result<bool> {
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_secs(20)))
        .open(port)
        .expect("Failed to open port. Are you running with root/admin privileges?");

    let fw = match fileops::read_file(filepath, FIRMWARE_SIZE) {
        Some(f) => f,
        _ => return Ok(false)   // Either None was returned or a panic was called
    };

    match uart::command_eraseflash(&port) {
        Ok(true) => println!("MCU flash erased"),
        _ => panic!("Failed to erase MCU flash. Ensure the radio is in bootloader mode.")
    }

    let mut offset = 0;

    while offset < FIRMWARE_SIZE {
        match uart::command_writeflash(&port, offset, &fw) {
            Ok(true) => print!("\rFlashing firmware to address {:#06x}", offset),
            _ => panic!("Failed to write firmware to MCU flash. Ensure your radio is firmly connected.")
        }
        offset += CHUNK_LENGTH
    }

    Ok(true)
}
