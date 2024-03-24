# rt890-flash

A flashing and dumping tool for Radtel's RT-890 radio transceiver, inspired by [DualTachyon's CLI flasher](https://github.com/DualTachyon/radtel-rt-890-flasher-cli). While it works under WINE, I wanted something that worked natively on GNU/Linux with the prospect of adding WebSerial functionality in future.

## Prerequisites

The latest stable Rust toolchain and your distro's equivalent `libudev` package, e.g. `libudev-devel` on Fedora (39), as needed by [serialport5](https://crates.io/crates/serialport5).

## Licence

This application is licenced under the Apache License, Version 2.0. See LICENSE or http://www.apache.org/licenses/LICENSE-2.0 for details.
