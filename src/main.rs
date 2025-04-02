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

mod helper;
mod radio;
mod uart;

#[cfg(unix)]
use helper::has_serial_access;

use radio::*;
use std::env::{self, args};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const COPYRIGHT: &str = "Copyright 2024-2025 bricky149 and contributors";
const HELP: &str = "Usage:
rt890-flash -l
rt890-flash -890 -p PORT -fw PATH
rt890-flash -4d -p PORT -fw/-dmr PATH
rt890-flash -890/-4d -p PORT -o/-w PATH

-l
List available ports that can be used.

-p PORT
Port to dump from or flash to.

-fw PATH
Flash radio with specified firmware file.
Radio MUST be in flash mode and will automatically restart.

-dmr PATH
Flash DMR chip with specified firmware file.
Radio MUST be in DMR update mode and be manually restarted.

-o PATH
Dump radio SPI flash to new file.
Radio MUST be in normal mode.

-w PATH
Restore specified dump file to radio SPI flash.
Radio MUST be in normal mode and be manually restarted.";

fn main() {
    // Always print so the user knows we are running
    println!("{}", COPYRIGHT);

    let args: Vec<String> = args().collect();
    match args.len() {
        2 => { // Executable name with one argument
            match args[1].as_str() {
                "-l" => {
                    println!("Ports available:");
                    for p in get_available_ports() {
                        println!("\t{}", p.port_name)
                    }
                }
                "--version" => println!("rt890-flash {}", VERSION),
                _ => println!("{}", HELP)
            }
        }
        6..=7 => { // Executable name with at least five arguments
            #[cfg(unix)]
            let user = env::var("USER").unwrap_or_default();
            #[cfg(unix)]
            if user != "root" && !has_serial_access(&user) {
                println!("Please add the current user to the dialout group or run the program as root");
                return
            }

            let is_890 = if args[1] == "-890" {
                true
            } else if args[1] == "-4d" || args[1] == "-4D" {
                // .to_uppercase and .to_lowercase adds 13kB of bloat
                false
            } else {
                println!("Please specify a radio model");
                return
            };

            match args[4].as_str() {
                "-fw" => {
                    match flash_mcu_firmware(&args[3], &args[5], is_890) {
                        Ok(true) => println!("\nRadio firmware flash complete. Radio should now reboot."),
                        Ok(false) => println!("Failed to flash radio firmware"),
                        Err(e) => eprintln!("{}. Ensure the radio is firmly connected and in flash mode.", e.description)
                    }
                }
                "-dmr" => {
                    if is_890 {
                        println!("The RT-890 does not have DMR firmware!");
                        return
                    }
                    match flash_dmr_firmware(&args[3], &args[5]) {
                        Ok(true) => println!("\nDMR firmware flash complete. Reboot the radio now."),
                        Ok(false) => println!("Failed to flash DMR firmware"),
                        Err(e) => eprintln!("{}. Ensure the radio is firmly connected and in DMR update mode.", e.description)
                    }
                }
                "-o" => {
                    match dump_spi_flash(&args[3], &args[5], is_890) {
                        Ok(true) => println!("\nSPI flash dump complete"),
                        Ok(false) => println!("Failed to read SPI flash"),
                        Err(e) => eprintln!("{}. Ensure the radio is firmly connected and turned on.", e.description)
                    }
                }
                "-w" => {
                    match restore_spi_flash(&args[3], &args[5], is_890) {
                        Ok(true) => println!("\nSPI flash restore complete. Reboot the radio now."),
                        Ok(false) => println!("Failed to write SPI flash"),
                        Err(e) => eprintln!("{}. Ensure the radio is firmly connected and turned on.", e.description)
                    }
                }
                _ => {
                    println!("Please specify a valid command")
                }
            }
        }
        _ => println!("{}", HELP)
    }
}
