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

use crate::{helper::create_padded_array, radio::SpiRange};
use std::io::{Read, Write};

pub enum CommandBytes {
    WriteDmrFlash = 0x05,
    EraseDmrFlash = 0x06,
    EraseFwFlash = 0x39,
    ReadSpiFlash = 0x52,
    UnlockFwFlash = 0x55,  // Undocumented, assuming firmware block is read-only if not passed 
    WriteFwFlash = 0x57
}

fn append_checksum(command: &mut [u8], read_only: bool) {
    let last_idx = command.len() - 1;
    let mut sum: u8 = 0;
    // Relies on arithmetic overflows
    for byte in command.iter().take(last_idx) {
        sum = sum.wrapping_add(*byte)
    }
    // If the buffer size is bigger than 132, it isn't an RT-890
    // Both radios send 4-byte packets when reading SPI flash
    if last_idx == 1027 || (last_idx != 131 && !read_only) {
        command[last_idx] = sum.wrapping_add(72)
    } else {
        command[last_idx] = sum
    }
}

fn verify_checksum(command: &[u8]) -> bool {
    let last_idx = command.len() - 1;
    let mut sum: u8 = 0;
    // Relies on arithmetic overflows
    for byte in command.iter().take(last_idx) {
        sum = sum.wrapping_add(*byte)
    }
    // If the buffer size is bigger than 132, it isn't an RT-890
    // Both radios send 4-byte packets when reading SPI flash
    if last_idx != 3 && last_idx != 131 {
        command[last_idx] == sum.wrapping_add(72)
    } else {
        command[last_idx] == sum
    }
}

pub fn command_erasemcuflash_890(mut port: &SerialPort) -> Result<bool> {
    let command = [
        CommandBytes::EraseFwFlash as u8,
        0,
        0,
        CommandBytes::UnlockFwFlash as u8,
        0x8E // Calculated sum of command bytes
    ];
    port.write_all(&command)?;

    let mut response = [0u8];
    port.read_exact(&mut response)?;
    match response {
        [0x06] => Ok(true),
        _ => Ok(false)
    }
}

pub fn command_erasemcuflash_4d(mut port: &SerialPort) -> Result<bool> {
    for block in 0x10..=0x55 {
        let mut command = [
            CommandBytes::EraseFwFlash as u8,
            0x33,
            0x05,
            block,
            0 // Calculated sum of command bytes
        ];
        append_checksum(&mut command, false);
        port.write_all(&command)?;

        let mut response = [0u8];
        port.read_exact(&mut response)?;
        match response {
            [0x06] => continue,
            _ => return Ok(false)
        }
    }

    Ok(true)
}

pub fn command_writemcuflash(mut port: &SerialPort, offset: usize, fw: &[u8], chunk_length: usize) -> Result<bool> {
    // RT-890 has a command buffer of size 132, RT-4D has one of size 1028 (4 + CHUNK_LENGTH)
    let mut command = create_padded_array(4+chunk_length);
    command[0] = CommandBytes::WriteFwFlash as u8;
    command[1] = ((offset >> 8) & 0xFF) as u8;
    command[2] = ((offset) & 0xFF) as u8;
    command[3..3+chunk_length].copy_from_slice(&fw[offset..offset+chunk_length]);

    // last command[] index reserved for checksum
    append_checksum(&mut command, false);
    port.write_all(&command)?;

    let mut response = [0u8];
    port.read_exact(&mut response)?;
    match response {
        [0x06] => Ok(true),
        _ => Ok(false)
    }
}

pub fn command_initdmrflash(mut port: &SerialPort) -> Result<bool> {
    // Unlike radio firmware updates, the RT-4D has to be intercepted
    // while it is entering DMR flash mode
    let command = [
        1,
        224,
        252,
        1,
        0
    ];
    port.write_all(&command)?;

    let mut response = [0u8; 80];
    let _bytes_read = port.read(&mut response)?;
    match response[4] {
        224 => Ok(true),
        _ => Ok(false)
    }
}

pub fn command_erasedmrflash(mut port: &SerialPort, flash_addr: usize) -> Result<bool> {
    let command = [
        1,
        224,
        252,
        255,
        244,
        CommandBytes::EraseDmrFlash as u8,
        0,
        15,
        (flash_addr & 0xFF) as u8,
        ((flash_addr >> 8) & 0xFF) as u8,
        ((flash_addr >> 16) & 0xFF) as u8,
        ((flash_addr >> 24) & 0xFF) as u8,
        0
    ];

    match port.write_all(&command) {
        Ok(()) => Ok(true),
        _ => Ok(false)
    }
}

pub fn command_writedmrflash(mut port: &SerialPort, flash_offset: usize, fw: &[u8], file_offset: usize) -> Result<bool> {
    let mut command = [0u8; 4108];
    command[0] = 1;
    command[1] = 224;
    command[2] = 252;
    command[3] = 255;
    command[4] = 244;
    command[5] = CommandBytes::WriteDmrFlash as u8;
    command[6] = 16;
    command[7] = 7;
    command[8] = 0;
    command[9] = (flash_offset & 0xFF) as u8;
    command[10] = ((flash_offset >> 8) & 0xFF) as u8;
    command[11] = 0;
    command[12..4108].copy_from_slice(&fw[file_offset..file_offset+4096]); // CHUNK_LENGTH on RT-4D

    match port.write_all(&command) {
        Ok(()) => Ok(true),
        _ => Ok(false)
    }
}

pub fn command_readspiflash(mut port: &SerialPort, offset: usize) -> Result<Option<Vec<u8>>> {
    let mut command = [0u8; 4];
    command[0] = CommandBytes::ReadSpiFlash as u8;
    command[1] = ((offset >> 8) & 0xFF) as u8;
    command[2] = (offset & 0xFF) as u8;

    append_checksum(&mut command, true);
    port.write_all(&command)?;

    let mut response = create_padded_array(1028);
    port.read_exact(&mut response)?;
    if !verify_checksum(&response) {
        // Probably an RT-890, saves us duplicating code
        response = create_padded_array(132);
        port.read_exact(&mut response)?;
    }

    if verify_checksum(&response) {
        // last command[] index reserved for checksum
        let eof_idx = response.len() - 2;
        let data = response[3..eof_idx].to_vec();
        return Ok(Some(data))
    }
    Ok(None)
}

pub fn command_writespiflash(mut port: &SerialPort, spi_range: &SpiRange, offset: usize, spi: &[u8]) -> Result<bool> {
    // TODO: Update to support RT-4D once relevant SPI ranges are published
    let block_offset = (offset - spi_range.offset) / 128;

    let mut command = [0u8; 132];
    command[0] = spi_range.cmd;
    command[1] = ((block_offset >> 8) & 0xFF) as u8;
    command[2] = (block_offset & 0xFF) as u8;
    command[3..130].copy_from_slice(&spi[offset..offset+128]); // CHUNK_LENGTH on RT-890

    // last command[] index reserved for checksum
    append_checksum(&mut command, false);
    port.write_all(&command)?;

    let mut response = [0u8];
    port.read_exact(&mut response)?;
    match response {
        [0x06] => Ok(true),
        _ => Ok(false)
    }
}
