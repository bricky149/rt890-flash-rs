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
use std::io::{stdout, Write};

pub const FW_890_SIZE: usize = 60_416;
const SPI_890_OFFSETS: [SpiRange; 9] = [
    SpiRange { cmd: SpiRegion890::EnglishPrompt as u8, offset: 0, size: 2949120 },
    SpiRange { cmd: SpiRegion890::EnglishAlphaNum as u8, offset: 2949120, size: 163840 },
    SpiRange { cmd: SpiRegion890::BigFont as u8, offset: 3112960, size: 139264 },
    SpiRange { cmd: SpiRegion890::SmallFont as u8, offset: 3252224, size: 8192 },
    SpiRange { cmd: SpiRegion890::ChinesePrompt as u8, offset: 3260416, size: 626688 },
    SpiRange { cmd: SpiRegion890::StartupLogo as u8, offset: 3887104, size: 40960 },
    SpiRange { cmd: SpiRegion890::Calibration as u8, offset: 3928064, size: 4096 },
    SpiRange { cmd: SpiRegion890::MemoriesAndSettings as u8, offset: 3936256, size: 40960},  // Does not pick up extended settings
    //SpiRange { cmd: SpiRegion890::ExtendedSettings as u8, offset: 4018176, size: 40960 },  // 0x3D5
    SpiRange { cmd: SpiRegion890::UnknownBlock as u8, offset: 4030464, size: 40960 }         // 0x3D8, possibly a bug as misses settings above
];

pub const DMR_FW_4D_SIZE: usize = 1_527_808;
pub const SPI_FLASH_SIZE: usize = 4_194_304;

const FW_4D_FLASH_SIZE: usize = 251_904;
const SPI_4D_OFFSETS: [SpiRange; 22] = [
    SpiRange { cmd: SpiRegion4D::Calibration as u8, offset: 0, size: 4096 },
    SpiRange { cmd: SpiRegion4D::Config as u8, offset: 8192, size: 4096 },
    SpiRange { cmd: SpiRegion4D::Channels as u8, offset: 16384, size: 49152 },
    SpiRange { cmd: SpiRegion4D::Zones as u8, offset: 114688, size: 131072 },
    SpiRange { cmd: SpiRegion4D::Contacts as u8, offset: 376832, size: 65536 },
    SpiRange { cmd: SpiRegion4D::Groups as u8, offset: 507904, size: 12288 },
    SpiRange { cmd: SpiRegion4D::EncryptKeys as u8, offset: 532480, size: 12288 },
    SpiRange { cmd: SpiRegion4D::CallLog as u8, offset: 557056, size: 49152 },
    SpiRange { cmd: SpiRegion4D::Sms as u8, offset: 606208, size: 4096 },
    SpiRange { cmd: SpiRegion4D::Schedules as u8, offset: 811008, size: 8192 },
    SpiRange { cmd: SpiRegion4D::FmTuner as u8, offset: 876544, size: 4096 },
    SpiRange { cmd: SpiRegion4D::StartupLogo as u8, offset: 877568, size: 4096 },
    SpiRange { cmd: SpiRegion4D::UnknownBlock1 as u8, offset: 1204224, size: 4096 },
    SpiRange { cmd: SpiRegion4D::UTF16MapTable as u8, offset: 1359872, size: 98304 },
    SpiRange { cmd: SpiRegion4D::ChinesePhonetics as u8, offset: 1458176, size: 147456 },
    SpiRange { cmd: SpiRegion4D::UnknownBlock2 as u8, offset: 1605632, size: 65536 },
    SpiRange { cmd: SpiRegion4D::UnknownBlock3 as u8, offset: 1671168, size: 16384 },
    SpiRange { cmd: SpiRegion4D::SevenBySixFonts as u8, offset: 1687552, size: 8192 },
    SpiRange { cmd: SpiRegion4D::MiscFonts as u8, offset: 1695744, size: 737280 },
    SpiRange { cmd: SpiRegion4D::UnknownBlock4 as u8, offset: 3420160, size: 61440 },
    SpiRange { cmd: SpiRegion4D::VoicePrompts as u8, offset: 3481600, size: 712704 },
    SpiRange { cmd: SpiRegion4D::AddressBook as u8, offset: 4194304, size: 12582912 }
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

pub enum SpiRegion890 {
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

pub enum SpiRegion4D {
    Calibration = 0x40,
    Config = 0x90,
    Channels = 0x91,
    Zones = 0x92,
    Contacts = 0x93,
    Groups = 0x94,
    EncryptKeys = 0x95,
    CallLog = 0x96,
    Sms = 0x97,
    Schedules = 0x98,
    FmTuner = 0x99,
    StartupLogo = 0x9A,
    UnknownBlock1 = 0x9B,
    UTF16MapTable = 0x9C,
    ChinesePhonetics = 0x9D,
    UnknownBlock2 = 0x9E,
    UnknownBlock3 = 0x9F,
    SevenBySixFonts = 0xA0,
    MiscFonts = 0xA1,
    UnknownBlock4 = 0xA2,
    VoicePrompts = 0xA3,
    AddressBook = 0xA4
}

#[derive(Clone)]
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
    let max_offset = if is_890 {
        32768 // * 128 = SPI_FLASH_SIZE
    } else {
        4096  // * 1024 = SPI_FLASH_SIZE
    };

    let mut request = RadioPacket::new(port_name, is_890);
    request.set_command(RadioCommand::ReadSpiFlash as u8);
    for offset in 0..max_offset {
        match request.read_spi_flash(offset) {
            Ok(Some(data)) => {
                print!("\rDumping SPI flash from address {:#x}", offset);
                spi.write_all(&data)?
            }
            Ok(None) => println!("Failed to dump from address {:#x}", offset),
            Err(e) => return Err(e)
        }
    }

    Ok(true)
}

pub fn restore_spi_flash(port_name: &str, file_path: &str, is_890: bool) -> Result<bool> {
    let spi = match helper::read_file(file_path, SPI_FLASH_SIZE) {
        Ok(f) => f,
        _ => return Ok(false)
    };
    let chunk_size = if is_890 {
        128
    } else {
        1024
    };
    let spi_offsets = if is_890 {
        SPI_890_OFFSETS.to_vec()
    } else {
        SPI_4D_OFFSETS.to_vec()
    };

    let mut request = RadioPacket::new(port_name, is_890);
    for spi_range in spi_offsets {
        request.set_command(spi_range.cmd);
        let mut offset = spi_range.offset;
        let block_length = offset + spi_range.size;
        
        while offset < block_length {
            let spi_data = &spi[offset..offset+chunk_size];
            match request.write_spi_flash(offset, spi_range.offset, spi_data) {
                Ok(true) => print!("\rRestoring SPI flash to address {:#x}", offset),
                Ok(false) => println!("Failed to restore to address {:#x}", offset),
                Err(e) => return Err(e)
            }
            offset += chunk_size
        }
    }

    Ok(true)
}

pub fn flash_mcu_firmware(port_name: &str, file_path: &str, is_890: bool) -> Result<bool> {
    let firmware_size = if is_890 {
        FW_890_SIZE
    } else {
        FW_4D_FLASH_SIZE
    };
    let fw = match helper::read_file(file_path, firmware_size) {
        Ok(f) => f,
        _ => return Ok(false)
    };
    let chunk_length = if is_890 {
        128
    } else {
        1024
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
            Ok(true) => print!("\rFlashing radio firmware to address {:#x}", offset),
            _ => return Ok(false)
        }
        offset += chunk_length
    }

    Ok(true)
}

pub fn flash_dmr_firmware(port_name: &str, file_path: &str) -> Result<bool> {
    let fw = match helper::read_file(file_path, DMR_FW_4D_SIZE) {
        Ok(f) => f,
        _ => return Ok(false)
    };
    let fw_crc = helper::calculate_crc(&fw[..]);
    println!("File CRC: {:#x}", fw_crc);

    // Wait for the radio to connect so we can intercept it
    // We cannot flash DMR firmware without doing this first
    print!("Waiting for radio...");
    stdout().flush()?;
    let mut request = DmrPacket::new(port_name);
    request.init_dmr_flash();

    for erase_offset in DMR_FW_4D_OFFSETS {
        request.erase_dmr_flash(erase_offset);
        print!("\rErasing DMR flash")
    }

    let mut file_offset = 0;
    for block in 0..373 {
        let page_offset = 272 + block * 16;
        let fw_data = &fw[file_offset..file_offset+4096];

        request.write_dmr_flash(page_offset, fw_data);
        print!("\rFlashing DMR firmware ({}/373)", block);
        stdout().flush()?;

        file_offset += 4096
    }

    let crc = request.get_dmr_crc();
    if crc - fw_crc != 0 {
        println!();
        println!("Checksum mismatch: got {:#x} from radio", crc);
        return Ok(false)
    }
    Ok(true)
}

pub fn get_available_ports() -> Vec<SerialPortInfo> {
    serialport5::available_ports().unwrap_or_default()
}
