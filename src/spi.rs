/*
    Copyright 2024-2025 Bricky
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

use crate::{helper, uart};
use std::io::Write;
use std::time::Duration;

const BAUD_RATE: u32 = 115_200;
const FW_4D_FLASH_SIZE: usize = 251_904;
pub const FW_890_SIZE: usize = 60_416;
pub const SPI_FLASH_SIZE: usize = 4_194_304;

pub enum FlashDataFlags {
    EnglishPrompt = 0x40,
    EnglishAlphaNum = 0x41,
    BigFont = 0x42,
    SmallFont = 0x43,
    StartupLogo = 0x47,
    Calibration = 0x48,
    MemoriesAndSettings = 0x49,
    UnknownBlock = 0x4B,         // Extended settings?
    ChinesePrompt = 0x4C
}

pub struct SpiRange {
    pub cmd: u8,
    pub offset: usize,
    size: usize
}

pub fn dump_spi_flash(port: &String, file_path: &String) {
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_secs(30)))
        .open(port)
        .expect("Failed to open port. Are you running with root/admin privileges?");

    let mut spi = match helper::create_file(file_path) {
        Some(f) => f,
        _ => return         // Panic already called from function
    };

    for offset in 0..32768 {
        match uart::command_readspiflash(&port, offset) {
            Ok(Some(data)) => {
                print!("\rDumping SPI flash from address {:#06x}", offset);
                spi.write_all(&data).expect("Failed to dump SPI flash")
            }
            Ok(None) => break,
            Err(e) => panic!("{}. Ensure the radio is in normal mode.", e)
        }
    }
}

pub fn restore_spi_flash(port: &String, calib_only: bool, file_path: &String) -> Result<bool> {
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_secs(30)))
        .open(port)
        .expect("Failed to open port. Are you running with root/admin privileges?");

    let spi = match helper::read_file_checked(file_path, SPI_FLASH_SIZE) {
        Some(f) => f,
        _ => return Ok(false)   // Either None was returned or a panic was called
    };

    let spi_ranges = if calib_only {
        vec![
            SpiRange { cmd: FlashDataFlags::Calibration as u8, offset: 3928064, size: 4096 }
        ]
    } else {
        vec![
            SpiRange { cmd: FlashDataFlags::EnglishPrompt as u8, offset: 0, size: 2949120 },
            SpiRange { cmd: FlashDataFlags::EnglishAlphaNum as u8, offset: 2949120, size: 163840 },
            SpiRange { cmd: FlashDataFlags::BigFont as u8, offset: 3112960, size: 139264 },
            SpiRange { cmd: FlashDataFlags::SmallFont as u8, offset: 3252224, size: 8192 },
            SpiRange { cmd: FlashDataFlags::ChinesePrompt as u8, offset: 3260416, size: 626688 },
            SpiRange { cmd: FlashDataFlags::StartupLogo as u8, offset: 3887104, size: 40960 },
            SpiRange { cmd: FlashDataFlags::Calibration as u8, offset: 3928064, size: 4096 },
            SpiRange { cmd: FlashDataFlags::MemoriesAndSettings as u8, offset: 3936256, size: 40960},  // Doesn't pick up extended settings
            //SpiRange { cmd: FlashDataFlags::ExtendedSettings as u8, offset: 4018176, size: 40960 },  // 0x3D5
            SpiRange { cmd: FlashDataFlags::UnknownBlock as u8, offset: 4030464, size: 40960 }         // 0x3D8, possibly a bug as misses settings above
        ]
    };

    for spi_range in spi_ranges {
        let mut offset = spi_range.offset;
        let block_length = offset + spi_range.size;

        while offset < block_length {
            match uart::command_writespiflash(&port, &spi_range, offset, &spi) {
                Ok(true) => print!("\rRestoring SPI flash to address {:#08x}", offset),
                _ => panic!("Failed to restore SPI flash. Ensure the radio is in normal mode.")
            }
            offset += 128 // CHUNK_LENGTH on RT-890
        }
    }

    Ok(true)
}

pub fn flash_firmware(port: &String, file_path: &String, check_size: bool) -> Result<bool> {
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_secs(30)))
        .open(port)
        .expect("Failed to open port. Are you running with root/admin privileges?");

    let chunk_length;
    let firmware_size;
    let fw = if check_size {
        // RT-890
        chunk_length = 128;
        firmware_size = FW_890_SIZE;
        match helper::read_file_checked(file_path, firmware_size) {
            Some(f) => f,
            _ => return Ok(false)   // Either None was returned or a panic was called
        }
    } else {
        // RT-4D
        chunk_length = 1024;
        firmware_size = FW_4D_FLASH_SIZE;
        match helper::read_file_padded(file_path, firmware_size) {
            Ok(f) => f,
            _ => return Ok(false)   // File read error was returned
        }
    };

    if check_size {
        match uart::command_eraseflash_890(&port) {
            Ok(true) => println!("MCU flash erased"),
            _ => panic!("Failed to erase MCU flash. Ensure the radio is in bootloader mode.")
        }
    } else {
        match uart::command_eraseflash_4d(&port) {
            Ok(true) => println!("MCU flash erased"),
            _ => panic!("Failed to erase MCU flash. Ensure the radio is in bootloader mode.")
        }
    }

    let mut offset = 0;
    while offset < firmware_size {
        match uart::command_writeflash(&port, offset, &fw, chunk_length) {
            Ok(true) => print!("\rFlashing firmware to address {:#06x}", offset),
            _ => panic!("Failed to write firmware to MCU flash. Ensure your radio is firmly connected.")
        }
        offset += chunk_length
    }

    Ok(true)
}
