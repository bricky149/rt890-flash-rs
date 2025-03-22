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

#[cfg(unix)]
extern crate nix;
#[cfg(unix)]
use nix::unistd::Uid;

#[cfg(windows)]
extern crate is_elevated;
#[cfg(windows)]
use is_elevated::is_elevated;

#[cfg(unix)]
pub fn runas_admin() -> bool {
    Uid::effective().is_root()
}

#[cfg(windows)]
pub fn runas_admin() -> bool {
    is_elevated()
}
