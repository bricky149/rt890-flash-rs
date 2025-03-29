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

use std::{fs::{self, File}, io::{self, Read}};

#[cfg(unix)]
pub fn has_serial_access(user: &str) -> bool {
    let group_file = fs::read_to_string("/etc/group")
        .expect("Unable to get available groups");

    // Find the "dialout" group and check if the user is listed
    group_file
        .lines()
        .find(|line| line.starts_with("dialout:"))
        .map(|line| line.contains(user))
        .unwrap_or(false)
}

pub fn create_padded_array(size: usize) -> Vec<u8> {
    // Create a zero-padded Vec of a set size
    // We cannot create an array with a size only known at run-time and
    // with_capacity creates an empty Vec, causing assignments to fail
    (0..size).map(|_| 0).collect()
}

pub fn read_file_checked(path: &str, expected_size: usize) -> Option<Vec<u8>> {
    // RT-890 expects firmware files to be an exact size
    // RT-4D expects DMR firmware files to be an exact size
    match fs::read(path) {
        Ok(f) => {
            if expected_size != 0 && f.len() != expected_size {
                return None
            }
            Some(f)
        },
        Err(_e) => None
    }
}

pub fn read_file_padded(file_path: &str, size: usize) -> io::Result<Vec<u8>> {
    // RT-4D radio firmware files can be of any size up to FW_4D_FLASH_SIZE
    // Padding it allows for the radio to reboot itself after flashing
    let mut buffer = create_padded_array(size);
    let mut file = File::open(file_path)?;
    let _bytes_read = file.read(&mut buffer[..])?;

    Ok(buffer)
}

pub fn create_file(path: &str) -> Option<File> {
    match File::create(path) {
        Ok(f) => Some(f),
        Err(_e) => None
    }
}
