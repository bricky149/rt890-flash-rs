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

pub const FW_890_SIZE: usize = 60_416;
pub const SPI_FLASH_SIZE: usize = 4_194_304;

const BAUD_RATE: u32 = 115_200;
const FW_4D_FLASH_SIZE: usize = 251_904;
const DMR_FW_4D_SIZE: usize = 1_527_808;

const DMR_FW_4D_OFFSETS: [usize; 43] = [
    17825824, 18874400, 19922976, 20971552, 22020128, 23068704, 24117280, 25165856, 26214432, 27263008,
    28311584, 29360160, 30408736, 31457312, 32505888, 33554648, 50331864, 67109080, 83886296, 100663512,
    117440728, 134217944, 150995160, 167772376, 184549592, 201326808, 218104024, 234881240, 251658456,
    268435672, 285212888, 301990104, 318767320, 335544536, 352321752, 369098968, 385876184, 402653216,
    403701792, 404750368, 405798944, 406847520, 407896096
];

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

pub fn flash_mcu_firmware(port: &String, file_path: &String, check_size: bool) -> Result<bool> {
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
        match uart::command_erasemcuflash_890(&port) {
            Ok(true) => println!("MCU firmware flash erased"),
            _ => panic!("Failed to erase radio firmware flash. Ensure the radio is in bootloader mode.")
        }
    } else {
        match uart::command_erasemcuflash_4d(&port) {
            Ok(true) => println!("MCU firmware flash erased"),
            _ => panic!("Failed to erase radio firmware flash. Ensure the radio is in bootloader mode.")
        }
    }

    let mut offset = 0;
    while offset < firmware_size {
        match uart::command_writemcuflash(&port, offset, &fw, chunk_length) {
            Ok(true) => print!("\rFlashing radio firmware to address {:#06x}", offset),
            _ => panic!("Failed to write radio firmware. Ensure your radio is firmly connected.")
        }
        offset += chunk_length
    }

    Ok(true)
}

pub fn flash_dmr_firmware(port: &String, file_path: &String) -> Result<bool> {
    // 10ms retry time as per original updater code
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_millis(10)))
        .open(port)
        .expect("Failed to open port. Are you running with root/admin privileges?");

    let fw = match helper::read_file_checked(file_path, DMR_FW_4D_SIZE) {
        Some(f) => f,
        _ => return Ok(false)   // Either None was returned or a panic was called
    };

    // Wait for the radio to connect so we can intercept it
    // We cannot flash DMR firmware without doing this first
    loop {
        match uart::command_initdmrflash(&port) {
            Ok(true) => break,
            _ => print!("\rWaiting for radio...")
        }
    }

    for erase_offset in DMR_FW_4D_OFFSETS {
        match uart::command_erasedmrflash(&port, erase_offset) {
            Ok(true) => print!("\rErasing DMR firmware flash"),
            _ => panic!("Failed to erase DMR firmware flash. Ensure your radio is firmly connected.")
        }
    }

    let mut fw_offset = 0;
    for block in 0..373 {
        let page_offset = 272 + block * 16;
        match uart::command_writedmrflash(&port, page_offset, &fw, fw_offset) {
            Ok(true) => print!("\rFlashing DMR firmware ({}/373)", block),
            _ => panic!("Failed to write DMR firmware. Ensure your radio is firmly connected.")
        }
        fw_offset += 4096
    }

    Ok(true)
}

pub fn get_available_ports() -> Vec<SerialPortInfo> {
    serialport5::available_ports().expect("No ports found")
}
