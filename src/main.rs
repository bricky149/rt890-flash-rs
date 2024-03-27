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

extern crate nix;
use nix::unistd::Uid;

extern crate serialport5;
use self::serialport5::*;

use std::env::args;
use std::fs::{self, File};
use std::io::Write;
use std::time::Duration;

mod uart;

const USAGE: &str = "
rt890-flash
Flashing and dumping tool for the Radtel RT-890.

rt890-flash -l
rt890-flash -p PORT -d FILE
rt890-flash -p PORT -f FILE

-l
List available ports, e.g. /dev/ttyUSB0

-p PORT
Port to read from, or write to.

-d FILE
Read external SPI flash to file, e.g. spi_backup.bin
Radio MUST be in normal mode.

-f FILE
Write firmware file to MCU flash, e.g. firmware.bin
Radio MUST be in bootloader mode and will automatically restart.
";

const BAUD_RATE: u32 = 115_200;
const CHUNK_LENGTH: usize = 128;
const FIRMWARE_SIZE: usize = 60_416;

fn dump_spi_flash(port: &String, filename: &String) {
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_secs(2)))
        .open(port)
        .expect("Failed to open port");

    let mut fw = match File::create(filename) {
        Ok(f) => f,
        Err(e) => panic!("{}", e)
    };

    for offset in 0..32768 {
        match uart::command_readspiflash(&port, offset) {
            Ok(Some(data)) => {
                print!("\rDumping SPI flash from address {:#08x}", offset);
                fw.write_all(&data).expect("Failed to dump SPI flash")
            }
            Ok(None) => break,
            Err(e) => panic!("{}. Is the radio in normal mode?", e)
        }
    }
}

fn flash_firmware(port: &String, filename: &String) -> Result<bool> {
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_secs(2)))
        .open(port)
        .expect("Failed to open port");

    let fw = match fs::read(filename) {
        Ok(f) => {
            if f.len() != FIRMWARE_SIZE {
                return Ok(false)
            };
            f
        },
        Err(e) => panic!("{}", e)
    };

    match uart::command_eraseflash(&port) {
        Ok(true) => println!("MCU flash erased"),
        _ => panic!("Failed to erase MCU flash. Is the radio in bootloader mode?")
    }

    let mut offset = 0;

    while offset < FIRMWARE_SIZE {
        match uart::command_writeflash(&port, offset, &fw) {
            Ok(true) => print!("\rFlashing firmware to address {:#06x}", offset),
            _ => panic!("Failed to write firmware to MCU flash")
        }
        offset += CHUNK_LENGTH
    }

    Ok(true)
}

fn main() {
    let args: Vec<String> = args().collect();
    match args.len() {
        2 => { // Executable name with one argument
            if args[1] != "-l" {
                println!("{}", USAGE);
                return
            }

            println!("Ports available:");
            for p in uart::get_available_ports() {
                println!("\t{}", p.port_name)
            }
        }
        5 => { // Executable name with four arguments
            if args[1] != "-p" {
                println!("{}", USAGE);
                return
            }

            if !Uid::effective().is_root() {
                println!("You must run this executable with root permissions");
                return
            }

            match args[3].as_str() {
                "-d" => {
                    dump_spi_flash(&args[2], &args[4]);
                    println!("\nSPI flash dump complete")
                }
                "-f" => {
                    match flash_firmware(&args[2], &args[4]) {
                        Ok(true) => println!("\nFirmware flash complete. Radio should now reboot."),
                        _ => println!("Specified file is not exactly {} bytes", FIRMWARE_SIZE)
                    }
                }
                _ => {
                    println!("{}", USAGE);
                }
            }
        }
        _ => println!("{}", USAGE)
    }
}
