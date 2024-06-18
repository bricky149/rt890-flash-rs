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

use std::env::args;
use std::fs::{self, File};
use std::io::Write;
use std::time::Duration;

mod spi;
use spi::SpiRange;

mod uart;

const HEADER: &str = "rt890-flash - Copyright 2024 bricky149";
const USAGE: &str = "Flashing and dumping tool for the Radtel RT-890.

rt890-flash -l
rt890-flash -p PORT -d FILE
rt890-flash -p PORT -f FILE
rt890-flash -p PORT -r [-c] FILE

-l
List available ports, e.g. /dev/ttyUSB0

-p PORT
Port to read from or write to.

-d FILE
Dump external SPI flash to file, e.g. spi_backup.bin
Radio MUST be in normal mode.

-f FILE
Write firmware file to MCU flash, e.g. firmware.bin
Radio MUST be in bootloader mode and will automatically restart.

-r [-c] FILE
Write flash dump to external SPI flash, e.g. spi_backup.bin
If -c is specified, only calibration data will be written.
Radio MUST be in normal mode and be manually restarted.
";

const BAUD_RATE: u32 = 115_200;
const CHUNK_LENGTH: usize = 128;
const FIRMWARE_SIZE: usize = 60_416;
const SPI_FLASH_SIZE: usize = 4_194_304;

fn dump_spi_flash(port: &String, filename: &String) {
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_secs(20)))
        .open(port)
        .expect("Failed to open port. Are you running with root/admin privileges?");

    let mut fw = match File::create(filename) {
        Ok(f) => f,
        Err(e) => panic!("{}", e)
    };

    for offset in 0..32768 {
        match uart::command_readspiflash(&port, offset) {
            Ok(Some(data)) => {
                print!("\rDumping SPI flash from address {:#06x}", offset);
                fw.write_all(&data).expect("Failed to dump SPI flash")
            }
            Ok(None) => break,
            Err(e) => panic!("{}. Ensure the radio is in normal mode.", e)
        }
    }
}

fn restore_spi_flash(port: &String, calib_only: bool, filename: &String) -> Result<bool> {
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_secs(20)))
        .open(port)
        .expect("Failed to open port. Are you running with root/admin privileges?");

    let spi = match fs::read(filename) {
        Ok(f) => {
            if f.len() != SPI_FLASH_SIZE {
                return Ok(false)
            };
            f
        },
        Err(e) => panic!("{}", e)
    };

    // TODO: Document these magic command bytes
    let spi_ranges;
    if calib_only {
        spi_ranges = vec![
            SpiRange { cmd: 0x48, offset: 3928064, size: 4096 }     // 3BF000 Calibration data
        ];
    } else {
        spi_ranges = vec![
            SpiRange { cmd: 0x40, offset: 0, size: 2949120 },
            SpiRange { cmd: 0x41, offset: 2949120, size: 163840 },
            SpiRange { cmd: 0x42, offset: 3112960, size: 139264 },
            SpiRange { cmd: 0x43, offset: 3252224, size: 8192 },
            SpiRange { cmd: 0x47, offset: 3887104, size: 40960 },
            SpiRange { cmd: 0x48, offset: 3928064, size: 4096 },    // 3BF000 Calibration data
            SpiRange { cmd: 0x49, offset: 3936256, size: 40960 },
            SpiRange { cmd: 0x4b, offset: 4030464, size: 40960 },
            SpiRange { cmd: 0x4c, offset: 3260416, size: 626688 }
        ]; 
    }

    for spi_range in spi_ranges {
        let mut offset = spi_range.offset;
        let block_length = offset + spi_range.size;

        while offset < block_length {
            match uart::command_writespiflash(&port, &spi_range, offset, &spi) {
                Ok(true) => print!("\rRestoring SPI flash to address {:#08x}", offset),
                _ => panic!("Failed to restore SPI flash. Ensure the radio is in normal mode.")
            }
            offset += CHUNK_LENGTH
        }
    }

    Ok(true)
}

fn flash_firmware(port: &String, filename: &String) -> Result<bool> {
    let port = SerialPort::builder()
        .baud_rate(BAUD_RATE)
        .read_timeout(Some(Duration::from_secs(20)))
        .open(port)
        .expect("Failed to open port. Are you running with root/admin privileges?");

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
        _ => panic!("Failed to erase MCU flash. Ensure the radio is in bootloader mode.")
    }

    let mut offset = 0;

    while offset < FIRMWARE_SIZE {
        match uart::command_writeflash(&port, offset, &fw) {
            Ok(true) => print!("\rFlashing firmware to address {:#06x}", offset),
            _ => panic!("Failed to write firmware to MCU flash. Ensure your radio is firmly connected.")
        }
        offset += CHUNK_LENGTH
    }

    Ok(true)
}

fn main() {
    // Always display header text
    println!("{}", HEADER);

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
        5..=6 => { // Executable name with four or five arguments
            if args[1] != "-p" {
                println!("{}", USAGE);
                return
            }

            // User may have port privileges, running as root/admin is not needed
            // https://chirpmyradio.com/projects/chirp/wiki/ChirpOnLinux#Serial-port-permissions

            match args[3].as_str() {
                "-d" => {
                    if args[4] != "-c" {
                        dump_spi_flash(&args[2], &args[4]);
                        println!("\nSPI flash dump complete")
                    } else {
                        // Cannot specify -c here
                        println!("{}", USAGE);
                        return
                    }
                }
                "-f" => {
                    if args[4] != "-c" {
                        match flash_firmware(&args[2], &args[4]) {
                            Ok(true) => println!("\nFirmware flash complete. Radio should now reboot."),
                            _ => println!("Specified file is not exactly {} bytes", FIRMWARE_SIZE)
                        }
                    } else {
                        // Cannot specify -c here
                        println!("{}", USAGE);
                        return
                    }
                }
                "-r" => {
                    if args[4] != "-c" {
                        match restore_spi_flash(&args[2], false, &args[4]) {
                            Ok(true) => println!("\nSPI flash restore complete. Reboot the radio now."),
                            _ => println!("Specified file is not exactly {} bytes", SPI_FLASH_SIZE)
                        }
                    } else {
                        match restore_spi_flash(&args[2], true, &args[5]) {
                            Ok(true) => println!("\nCalibration restore complete. Reboot the radio now."),
                            _ => println!("Specified file is not exactly {} bytes", SPI_FLASH_SIZE)
                        }
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
