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

mod uart;

extern crate nix;
use nix::unistd::Uid;

use std::env::args;
use std::fs::{self, File};
use std::io::Write;

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
Read flash to file, e.g. backup.bin
Radio MUST be in normal mode.

-f FILE
Write file to flash, e.g. firmware.bin
Radio MUST be in bootloader mode.
";

fn dump_flash(port: &String, filename: &String) {
    let mut fw = match File::create(filename) {
        Ok(f) => f,
        Err(e) => panic!("{}", e)
    };

    let mut i = 0;

    while i < 65535 {
        match uart::command_readflash(port, i) {
            Ok(Some(data)) => {
                print!("\rDumping from {:#04x}", i);
                fw.write_all(&data).expect("Failed to dump flash")
            }
            Ok(None) => break,
            Err(e) => panic!("{}. Is the radio in normal mode?", e)
        }
        i += 128
    }
}

fn flash_firmware(port: &String, filename: &String) {
    match uart::command_eraseflash(port) {
        Ok(true) => println!("Flash erased"),
        _ => panic!("Failed to erase flash. Is the radio in bootloader mode?")
    }

    let fw = match fs::read(filename) {
        Ok(f) => f,
        Err(e) => panic!("{}", e)
    };

    let fw_size = fw.len();
    let mut i = 0;
    
    while i < fw_size {
        match uart::command_writeflash(port, i, &fw) {
            Ok(true) => print!("\rFlashing to {:#04x}", i),
            _ => panic!("Failed to flash firmware")
        }
        i += 128
    }
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
                    dump_flash(&args[2], &args[4]);
                    println!("\nDump complete")
                }
                "-f" => {
                    flash_firmware(&args[2], &args[4]);
                    println!("\nFlash complete")
                }
                _ => {
                    println!("{}", USAGE);
                }
            }
        }
        _ => println!("{}", USAGE)
    }
}
