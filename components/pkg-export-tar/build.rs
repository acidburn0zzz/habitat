// Inline common build behavior
include!("../libbuild.rs");

use std::env;

fn main() {
    habitat::common();
    write_tar_pkg_ident();
}

fn write_tar_pkg_ident() {
    let ident = match env::var("PLAN_TAR_PKG_IDENT") {
        // Use the value provided by the build system if present
        Ok(ident) => ident,
        // Use the latest installed package as a default for development
        _ => String::from("core/tar"),
    };
    util::write_out_dir_file("TAR_PKG_IDENT", ident);
}

