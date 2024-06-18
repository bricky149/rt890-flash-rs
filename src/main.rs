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

mod fileops;
mod spi;
mod uart;

use spi::*;
use std::env::args;

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
