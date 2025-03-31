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

use crate::helper;
use crate::uart::{DmrPacket, RadioPacket};
use std::io::Write;

pub const FW_890_SIZE: usize = 60_416;
const SPI_890_OFFSETS: [SpiRange; 9] = [
    SpiRange { cmd: SpiFlags::EnglishPrompt as u8, offset: 0, size: 2949120 },
    SpiRange { cmd: SpiFlags::EnglishAlphaNum as u8, offset: 2949120, size: 163840 },
    SpiRange { cmd: SpiFlags::BigFont as u8, offset: 3112960, size: 139264 },
    SpiRange { cmd: SpiFlags::SmallFont as u8, offset: 3252224, size: 8192 },
    SpiRange { cmd: SpiFlags::ChinesePrompt as u8, offset: 3260416, size: 626688 },
    SpiRange { cmd: SpiFlags::StartupLogo as u8, offset: 3887104, size: 40960 },
    SpiRange { cmd: SpiFlags::Calibration as u8, offset: 3928064, size: 4096 },
    SpiRange { cmd: SpiFlags::MemoriesAndSettings as u8, offset: 3936256, size: 40960},  // Does not pick up extended settings
    //SpiRange { cmd: SpiFlags::ExtendedSettings as u8, offset: 4018176, size: 40960 },  // 0x3D5
    SpiRange { cmd: SpiFlags::UnknownBlock as u8, offset: 4030464, size: 40960 }         // 0x3D8, possibly a bug as misses settings above
];

pub const DMR_FW_4D_SIZE: usize = 1_527_808;
pub const SPI_FLASH_SIZE: usize = 4_194_304;

const FW_4D_FLASH_SIZE: usize = 251_904;
const SPI_4D_OFFSETS: [SpiRange; 9] = [
    SpiRange { cmd: SpiFlags::Config as u8, offset: 0, size: 12288 },
    SpiRange { cmd: SpiFlags::Channels as u8, offset: 16384, size: 49152 },
    SpiRange { cmd: SpiFlags::Zones as u8, offset: 114688, size: 131072 },
    SpiRange { cmd: SpiFlags::Contacts as u8, offset: 376832, size: 65536 },
    SpiRange { cmd: SpiFlags::GroupLists as u8, offset: 507904, size: 12288 },
    SpiRange { cmd: SpiFlags::Encryption as u8, offset: 532480, size: 12288 },
    SpiRange { cmd: SpiFlags::SmsData as u8, offset: 606208, size: 204800 },
    SpiRange { cmd: SpiFlags::FmTuner as u8, offset: 876544, size: 1024 },
    SpiRange { cmd: SpiFlags::StartPic as u8, offset: 877568, size: 1024 }
];
const DMR_FW_4D_OFFSETS: [usize; 43] = [
    17825824, 18874400, 19922976, 20971552, 22020128, 23068704, 24117280, 25165856, 26214432, 27263008,
    28311584, 29360160, 30408736, 31457312, 32505888, 33554648, 50331864, 67109080, 83886296, 100663512,
    117440728, 134217944, 150995160, 167772376, 184549592, 201326808, 218104024, 234881240, 251658456,
    268435672, 285212888, 301990104, 318767320, 335544536, 352321752, 369098968, 385876184, 402653216,
    403701792, 404750368, 405798944, 406847520, 407896096
];

enum RadioCommand {
    EraseFwFlash = 0x39,
    ReadSpiFlash = 0x52,
    WriteFwFlash = 0x57
}

enum DmrCommand {
    InitFlash = 0x00,
    WriteFlash = 0x05,
    EraseFlash = 0x06
}

pub enum SpiFlags {
    // RT-890
    EnglishPrompt = 0x40,
    EnglishAlphaNum = 0x41,
    BigFont = 0x42,
    SmallFont = 0x43,
    StartupLogo = 0x47,
    Calibration = 0x48,
    MemoriesAndSettings = 0x49,
    UnknownBlock = 0x4B,         // Extended settings?
    ChinesePrompt = 0x4C,
    // RT-4D
    Config = 0x90,
    Channels = 0x91,
    Zones = 0x92,
    Contacts = 0x93,
    GroupLists = 0x94,
    Encryption = 0x95,
    SmsData = 0x97,
    FmTuner = 0x99,
    StartPic = 0x9A
}

pub struct SpiRange {
    pub cmd: u8,
    pub offset: usize,
    size: usize
}

pub fn dump_spi_flash(port_name: &str, file_path: &str, is_890: bool) -> Result<bool> {
    let mut spi = match helper::create_file(file_path) {
        Some(f) => f,
        _ => return Ok(false)
    };

    let mut request = RadioPacket::new(port_name, is_890);
    request.set_command(RadioCommand::ReadSpiFlash as u8);

    let max_offset = if is_890 {
        32768 // * 128 = SPI_FLASH_SIZE
    } else {
        4096  // * 1024 = SPI_FLASH_SIZE
    };

    for offset in 0..max_offset {
        match request.read_spi_flash(offset) {
            Ok(Some(data)) => {
                print!("\rDumping SPI flash from address {:#06x}", offset);
                spi.write_all(&data)?
            }
            Ok(None) => println!("Failed to dump from address {:#06x}", offset),
            Err(e) => return Err(e)
        }
    }

    Ok(true)
}

pub fn restore_spi_flash(port_name: &str, file_path: &str, is_890: bool) -> Result<bool> {
    let spi = match helper::read_file_checked(file_path, SPI_FLASH_SIZE) {
        Some(f) => f,
        _ => return Ok(false)
    };

    let chunk_size = if is_890 {
        128
    } else {
        1024
    };
    let spi_offsets = if is_890 {
        SPI_890_OFFSETS
    } else {
        SPI_4D_OFFSETS
    };
    let mut request = RadioPacket::new(port_name, is_890);

    for spi_range in spi_offsets {
        request.set_command(spi_range.cmd);
        let mut offset = spi_range.offset;
        let block_length = offset + spi_range.size;
        
        while offset < block_length {
            let spi_data = &spi[offset..offset+chunk_size];
            match request.write_spi_flash(offset, spi_range.offset, spi_data) {
                Ok(true) => print!("\rRestoring SPI flash to address {:#08x}", offset),
                Ok(false) => println!("Failed to restore to address {:#06x}", offset),
                Err(e) => return Err(e)
            }
            offset += chunk_size
        }
    }

    Ok(true)
}

pub fn flash_mcu_firmware(port_name: &str, file_path: &str, is_890: bool) -> Result<bool> {
    let chunk_length;
    let firmware_size;

    let fw = if is_890 {
        chunk_length = 128;
        firmware_size = FW_890_SIZE;
        match helper::read_file_checked(file_path, firmware_size) {
            Some(f) => f,
            _ => return Ok(false)
        }
    } else {
        chunk_length = 1024;
        firmware_size = FW_4D_FLASH_SIZE;
        match helper::read_file_padded(file_path, firmware_size) {
            Ok(f) => f,
            Err(e) => {
                // Print here as we cannot propagate up without casting
                eprintln!("{}", e);
                return Ok(false)
            }
        }
    };

    let mut request = RadioPacket::new(port_name, is_890);
    request.set_command(RadioCommand::EraseFwFlash as u8);
    match request.erase_mcu_flash(is_890) {
        Ok(true) => println!("MCU firmware flash erased"),
        _ => return Ok(false)
    }

    request.set_command(RadioCommand::WriteFwFlash as u8);
    let mut offset = 0;
    while offset < firmware_size {
        let fw_data = &fw[offset..offset+chunk_length];
        match request.write_mcu_flash(offset, fw_data) {
            Ok(true) => print!("\rFlashing radio firmware to address {:#06x}", offset),
            _ => return Ok(false)
        }
        offset += chunk_length
    }

    Ok(true)
}

pub fn flash_dmr_firmware(port_name: &str, file_path: &str) -> Result<bool> {
    let fw = match helper::read_file_checked(file_path, DMR_FW_4D_SIZE) {
        Some(f) => f,
        _ => return Ok(false)
    };

    // Wait for the radio to connect so we can intercept it
    // We cannot flash DMR firmware without doing this first
    let mut request = DmrPacket::new(port_name);
    request.set_command(DmrCommand::InitFlash as u8);
    loop {
        match request.init_dmr_flash() {
            Ok(true) => break,
            _ => print!("\rWaiting for radio...")
        }
    }

    request.set_command(DmrCommand::EraseFlash as u8);
    for erase_offset in DMR_FW_4D_OFFSETS {
        match request.erase_dmr_flash(erase_offset) {
            Ok(true) => print!("\rErasing DMR firmware flash"),
            _ => return Ok(false)
        }
    }

    request.set_command(DmrCommand::WriteFlash as u8);
    let mut file_offset = 0;
    for block in 0..373 {
        let page_offset = 272 + block * 16;
        let fw_data = &fw[file_offset..file_offset+4096];
        match request.write_dmr_flash(page_offset, fw_data) {
            Ok(true) => print!("\rFlashing DMR firmware ({}/373)", block),
            _ => return Ok(false)
        }
        file_offset += 4096
    }

    Ok(true)
}

pub fn get_available_ports() -> Vec<SerialPortInfo> {
    serialport5::available_ports().unwrap_or_default()
}
