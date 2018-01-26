// Copyright (c) 2016-2017 Chef Software Inc. and/or applicable contributors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::path::Path;
use std::fs::{self, File};
use std::io::Write;
use error::Result;

const BIN_PATH: &'static str = "/bin";

/// Returns the `bin` path used for symlinking programs.
pub fn bin_path() -> &'static Path {
    Path::new(BIN_PATH)
}

/// Writes a truncated/new file at the provided path with the provided content.
///
/// # Errors
///
/// * If an `IO` error occurs while creating, tuncating, writing, or closing the file
pub fn write_file<T>(file: T, content: &str) -> Result<()>
where
    T: AsRef<Path>,
{
    fs::create_dir_all(file.as_ref().parent().expect("Parent directory exists"))?;
    let mut f = File::create(file)?;
    f.write_all(content.as_bytes())?;
    Ok(())
}

