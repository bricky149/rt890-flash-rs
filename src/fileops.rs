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

use std::fs::{self, File};

pub fn read_file(path: &String, expected_size: usize) -> Option<Vec<u8>> {
    match fs::read(path) {
        Ok(f) => {
            if f.len() != expected_size {
                return None
            };
            return Some(f)
        },
        Err(e) => panic!("{}", e)
    };
}

pub fn create_file(path: &String) -> Option<File> {
    match File::create(path) {
        Ok(f) => Some(f),
        Err(e) => panic!("{}", e)
    }
}
