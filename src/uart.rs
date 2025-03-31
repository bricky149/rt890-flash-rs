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

use crate::helper::create_padded_array;
use std::{io::{Read, Write}, process::exit, time::Duration};

const BAUD_RATE: u32 = 115_200;

pub struct RadioPacket {
    port: SerialPort,
    buffer: Vec<u8>
}

impl RadioPacket {
    pub fn new(port_name: &str, is_890: bool) -> Self {
        Self {
            port: SerialPort::builder()
                .baud_rate(BAUD_RATE)
                .read_timeout(Some(Duration::from_secs(25)))
                .open(port_name)
                .unwrap_or_else(|e| {
                    eprintln!("{}", e.description);
                    exit(0)
                }),
            buffer: if is_890 {
                create_padded_array(132)
            } else {
                create_padded_array(1028)
            }
        }
    }

    pub fn set_command(&mut self, command: u8) {
        self.buffer[0] = command
    }

    fn append_checksum(&mut self, initial_seed: u8, sum_index: usize) {
        let mut sum = initial_seed;
        // Relies on arithmetic overflows
        for &byte in &self.buffer[..sum_index] {
            sum = sum.wrapping_add(byte)
        }
        self.buffer[sum_index] = sum
    }

    fn verify_checksum(&self, initial_seed: u8, sum_index: usize) -> bool {
        let mut sum = initial_seed;
        // Relies on arithmetic overflows
        for &byte in &self.buffer[..sum_index] {
            sum = sum.wrapping_add(byte)
        }
        self.buffer[sum_index] == sum
    }

    pub fn erase_mcu_flash(&mut self, is_890: bool) -> Result<bool> {
        // Undocumented, assuming access is read-only if not passed 
        self.buffer[3] = 0x55;
        // last index reserved for checksum
        if is_890 {
            self.append_checksum(0, 4)
        } else {
            self.append_checksum(72, 4)
        }
        self.port.write_all(&self.buffer[..5])?;

        let mut response = [0u8];
        self.port.read_exact(&mut response)?;
        match response {
            [0x06] => Ok(true),
            _ => Ok(false)
        }
    }

    pub fn write_mcu_flash(&mut self, offset: usize, fw_data: &[u8]) -> Result<bool> {
        let chunk_length = fw_data.len() + 3;
        
        self.buffer[1] = ((offset >> 8) & 0xFF) as u8;
        self.buffer[2] = ((offset) & 0xFF) as u8;
        self.buffer[3..chunk_length].copy_from_slice(fw_data);
        // last index reserved for checksum
        if chunk_length == 131 {
            self.append_checksum(0, chunk_length);
        } else {
            self.append_checksum(72, chunk_length);
        }
        self.port.write_all(&self.buffer)?;
    
        let mut response = [0u8];
        self.port.read_exact(&mut response)?;
        match response {
            [0x06] => Ok(true),
            _ => Ok(false)
        }
    }

    pub fn read_spi_flash(&mut self, offset: usize) -> Result<Option<Vec<u8>>> {
        self.buffer[1] = ((offset >> 8) & 0xFF) as u8;
        self.buffer[2] = (offset & 0xFF) as u8;
        // last index reserved for checksum
        self.append_checksum(0, 3);
        self.port.write_all(&self.buffer[..4])?;

        self.port.read_exact(&mut self.buffer)?;
        // Returns the number of elements
        // 132 for the RT-890, 1028 for the RT-4D
        let sum_index = self.buffer.len() - 1;
        if self.verify_checksum(0, sum_index) {
            // Exclude checksum as it is not part of the data
            let data = self.buffer[3..sum_index].to_vec();
            return Ok(Some(data))
        }

        Ok(None)
    }

    pub fn write_spi_flash(&mut self, offset: usize, spi_offset: usize, spi_data: &[u8]) -> Result<bool> {
        let chunk_length = spi_data.len() + 3;
        let block_offset = (offset - spi_offset) / chunk_length;

        self.buffer[1] = ((block_offset >> 8) & 0xFF) as u8;
        self.buffer[2] = (block_offset & 0xFF) as u8;
        self.buffer[3..chunk_length].copy_from_slice(spi_data);
        // last index reserved for checksum
        self.append_checksum(0, chunk_length);
        self.port.write_all(&self.buffer)?;
    
        let mut response = [0u8];
        self.port.read_exact(&mut response)?;
        match response {
            [0x06] => Ok(true),
            _ => Ok(false)
        }
    }
}

pub struct DmrPacket {
    port: SerialPort,
    buffer: [u8; 4108]
}

impl DmrPacket {
    pub fn new(port_name: &str) -> Self {
        Self {
            // 10ms as per original updater code
            port: SerialPort::builder()
                .baud_rate(BAUD_RATE)
                .read_timeout(Some(Duration::from_millis(10)))
                .open(port_name)
                .unwrap_or_else(|e| {
                    eprintln!("{}", e.description);
                    exit(0)
                }),
            buffer: [0u8; 4108]
        }
    }

    pub fn set_command(&mut self, command: u8) {
        self.buffer[5] = command
    }

    pub fn init_dmr_flash(&mut self) -> Result<bool> {
        // Unlike radio firmware updates, the RT-4D has to be intercepted
        // while it is entering DMR flash mode
        self.buffer[0] = 1;
        self.buffer[1] = 224;
        self.buffer[2] = 252;
        self.buffer[3] = 1;
        self.port.write_all(&self.buffer[..5])?;

        let mut response = [0u8; 5];
        let _bytes_read = self.port.read(&mut response)?;
        match response[4] {
            224 => Ok(true),
            _ => Ok(false)
        }
    }

    pub fn erase_dmr_flash(&mut self, offset: usize) -> Result<bool> {
        self.buffer[3] = 255;
        self.buffer[4] = 244;
        self.buffer[7] = 15;
        self.buffer[8] = (offset & 0xFF) as u8;
        self.buffer[9] = ((offset >> 8) & 0xFF) as u8;
        self.buffer[10] = ((offset >> 16) & 0xFF) as u8;
        self.buffer[11] = ((offset >> 24) & 0xFF) as u8;

        match self.port.write_all(&self.buffer[..12]) {
            Ok(()) => Ok(true),
            _ => Ok(false)
        }
    }

    pub fn write_dmr_flash(&mut self, offset: usize, fw_data: &[u8]) -> Result<bool> {
        self.buffer[4] = 244;
        self.buffer[6] = 16;
        self.buffer[7] = 7;
        self.buffer[8] = 0;
        self.buffer[9] = (offset & 0xFF) as u8;
        self.buffer[10] = ((offset >> 8) & 0xFF) as u8;
        self.buffer[11] = 0;
        self.buffer[12..].copy_from_slice(fw_data);

        match self.port.write_all(&self.buffer) {
            Ok(()) => Ok(true),
            _ => Ok(false)
        }
    }
}
