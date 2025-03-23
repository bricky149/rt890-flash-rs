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

use crate::{helper::create_dynamic_array, spi::SpiRange};
use std::io::{Read, Write};

pub enum CommandBytes {
    EraseFwFlash = 0x39,
    ReadSpiFlash = 0x52,
    UnlockFwFlash = 0x55,  // Undocumented, assuming firmware block is read-only if not passed 
    WriteFwFlash = 0x57
}

//pub enum ResponseBytes {
//  AckResponse = 0x06,
//  FlashModeResponse = 0xFF
//}

fn append_checksum(command: &mut [u8]) {
    let last_idx = command.len() - 1;
    let mut sum: u8 = 0;
    // Relies on arithmetic overflows
    for byte in command.iter().take(last_idx) {
        sum = sum.wrapping_add(*byte)
    }
    // If the buffer size is bigger than 132, it isn't an RT-890
    // Both radios send 4-byte packets when reading SPI flash
    if last_idx != 3 && last_idx != 1027 {
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
    if last_idx != 3 && last_idx != 1027 {
        command[last_idx] == sum.wrapping_add(72)
    } else {
        command[last_idx] == sum
    }
}

pub fn command_eraseflash_890(mut port: &SerialPort) -> Result<bool> {
    let command = [
        CommandBytes::EraseFwFlash as u8,
        0,
        0,
        CommandBytes::UnlockFwFlash as u8,
        0x8E // Checksum of command bytes
    ];
    port.write_all(&command)?;

    let mut response = [0u8];
    port.read_exact(&mut response)?;
    match response {
        [0x06] => Ok(true),
        _ => Ok(false)
    }
}

pub fn command_eraseflash_4d(mut port: &SerialPort) -> Result<bool> {
    let mut block = 0x10;

    while block <= 0x55 {
        let mut command = [
            CommandBytes::EraseFwFlash as u8,
            0x33,
            0x05,
            block,
            0 // Checksum of command bytes
        ];
        append_checksum(&mut command);
        port.write_all(&command)?;

        let mut response = [0u8];
        port.read_exact(&mut response)?;
        match response {
            [0x06] => {
                block += 1
            },
            _ => return Ok(false)
        }
    }

    Ok(true)
}

pub fn command_writeflash(mut port: &SerialPort, offset: usize, fw: &[u8], chunk_length: usize) -> Result<bool> {
    // RT-890 has a command buffer of size 132, RT-4D has one of size 1028 (4 + CHUNK_LENGTH)
    let mut command = create_dynamic_array(4+chunk_length);
    command[0] = CommandBytes::WriteFwFlash as u8;
    command[1] = ((offset >> 8) & 0xFF) as u8;
    command[2] = ((offset) & 0xFF) as u8;

    // Prevent from reading out-of-bounds
    if offset+chunk_length < fw.len() {
        command[3..chunk_length+3].copy_from_slice(&fw[offset..offset+chunk_length]);
    } else {
        let final_chunk = fw.len() - offset;
        command[3..final_chunk+3].copy_from_slice(&fw[offset..offset+final_chunk]);
    }
    // last command[] index reserved for checksum
    append_checksum(&mut command);
    port.write_all(&command)?;

    let mut response = [0u8];
    port.read_exact(&mut response)?;
    match response {
        [0x06] => Ok(true),
        _ => Ok(false)
    }
}

pub fn command_readspiflash(mut port: &SerialPort, offset: usize) -> Result<Option<Vec<u8>>> {
    let mut command = [0u8; 4];
    command[0] = CommandBytes::ReadSpiFlash as u8;
    command[1] = ((offset >> 8) & 0xFF) as u8;
    command[2] = ((offset) & 0xFF) as u8;

    append_checksum(&mut command);
    port.write_all(&command)?;

    let mut response = create_dynamic_array(1028);
    port.read_exact(&mut response)?;
    if !verify_checksum(&response) {
        // Probably an RT-890, saves us duplicating code
        response = create_dynamic_array(132);
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
    let block_offset = (offset - spi_range.offset) / 128;

    let mut command = [0u8; 132];
    command[0] = spi_range.cmd;
    command[1] = ((block_offset >> 8) & 0xFF) as u8;
    command[2] = ((block_offset) & 0xFF) as u8;
    command[3..130].copy_from_slice(&spi[offset..offset+128]); // CHUNK_LENGTH on RT-890

    // last command[] index reserved for checksum
    append_checksum(&mut command);
    port.write_all(&command)?;

    let mut response = [0u8];
    port.read_exact(&mut response)?;
    match response {
        [0x06] => Ok(true),
        _ => Ok(false)
    }
}

pub fn get_available_ports() -> Vec<SerialPortInfo> {
    serialport5::available_ports().expect("No ports found")
}
