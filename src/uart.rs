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

use crate::helper::*;
use crate::spi::SpiRange;
use std::io::{Read, Write};

pub enum CommandBytes {
    CmdEraseFwFlash = 0x39,
    CmdReadSpiFlash = 0x52,
    CmdUnlockFwFlash = 0x55,  // Undocumented, assuming firmware block is read-only if not passed 
    CmdWriteFwFlash = 0x57
}

//pub enum ResponseBytes {
//  AckResponse = 0x06,
//  FlashModeResponse = 0xFF
//}

fn checksum(command: &mut [u8]) {
    let last_idx = command.len() - 1;
    let mut sum = 0;
    // Relies on arithmetic overflows
    for byte in command.iter().take(last_idx) {
        sum += byte
    }
    let checksum = (sum as u16 + 72) % 256;
    command[last_idx] = checksum as u8
}

fn verify(command: &[u8]) -> bool {
    let last_idx = command.len() - 1;
    let mut calculated_sum = 0;
    // Relies on arithmetic overflows
    for byte in command.iter().take(last_idx) {
        calculated_sum += byte
    }
    command[last_idx] == calculated_sum
}

pub fn command_eraseflash_890(mut port: &SerialPort) -> Result<bool> {
    let mut command = [0u8; 5];
    command[0] = CommandBytes::CmdEraseFwFlash as u8;
    command[3] = CommandBytes::CmdUnlockFwFlash as u8;

    checksum(&mut command);
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
        let mut command = [0u8; 5];
        command[0] = CommandBytes::CmdEraseFwFlash as u8;
        command[1] = 0x33;
        command[2] = 0x05;
        command[3] = block;

        checksum(&mut command);
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
    command[0] = CommandBytes::CmdWriteFwFlash as u8;
    command[1] = ((offset >> 8) & 0xFF) as u8;
    command[2] = ((offset) & 0xFF) as u8;

    // Prevent us from reading out-of-bounds
    if offset+chunk_length < fw.len() {
        command[3..3+chunk_length].copy_from_slice(&fw[offset..offset+chunk_length]);
    } else {
        let final_chunk = fw.len() - offset;
        command[3..3+final_chunk].copy_from_slice(&fw[offset..offset+final_chunk]);
    }

    checksum(&mut command);
    port.write_all(&command)?;

    let mut response = [0u8];
    port.read_exact(&mut response)?;
    match response {
        [0x06] => Ok(true),
        _ => Ok(false)
    }
}

pub fn command_readspiflash(mut port: &SerialPort, offset: u16) -> Result<Option<Vec<u8>>> {
    let mut command = [0u8; 4];
    command[0] = CommandBytes::CmdReadSpiFlash as u8;
    command[1] = ((offset >> 8) & 0xFF) as u8;
    command[2] = ((offset) & 0xFF) as u8;

    checksum(&mut command);
    port.write_all(&command)?;

    let mut block = [0u8; 132];
    port.read_exact(&mut block)?;
    if !verify(&block) {
        // Sometimes returns no data on first run
        port.read_exact(&mut block)?;
    }

    if verify(&block) {
        let data = block[3..131].to_vec();
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
    command[3..131].copy_from_slice(&spi[offset..offset+128]); // CHUNK_LENGTH on RT-890

    checksum(&mut command);
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
