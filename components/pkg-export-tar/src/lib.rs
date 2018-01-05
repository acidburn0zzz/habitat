#[macro_use]
extern crate clap;
extern crate habitat_core as hcore;
extern crate url;
extern crate habitat_common as common;
extern crate base64;

extern crate hab;
extern crate handlebars;

extern crate mktemp;
extern crate tempdir;
extern crate failure;
#[macro_use]
extern crate failure_derive;
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate serde_json;
extern crate regex;

mod build;
pub mod cli;
mod error;
mod util;

use std::process::Command;
use std::path::{Path, PathBuf};
use std::str::FromStr;
pub use cli::{Cli, PkgIdentArgOptions};
pub use error::{Error, Result};
use common::ui::UI;
use hcore::channel;
use hcore::url as hurl;
use hcore::package::{PackageIdent, PackageArchive};
use mktemp::Temp;
use regex::Regex;

pub use build::BuildSpec;

/// The version of this library and program when built.
pub const VERSION: &'static str = include_str!(concat!(env!("OUT_DIR"), "/VERSION"));

/// An image naming policy.
///
/// This is a value struct which captures the naming and tagging intentions for an image.
#[derive(Debug)]
pub struct Naming<'a> {
    /// An optional custom image name which would override a computed default value.
    pub custom_image_name: Option<&'a str>,
    /// Whether or not to tag the image with a latest value.
    pub latest_tag: bool,
    /// Whether or not to tag the image with a value containing a version from a Package
    /// Identifier.
    pub version_tag: bool,
    /// Whether or not to tag the image with a value containing a version and release from a
    /// Package Identifier.
    pub version_release_tag: bool,
    /// An optional custom tag value for the image.
    pub custom_tag: Option<&'a str>,
}

impl<'a> Naming<'a> {
    /// Creates a `Naming` from cli arguments.
    pub fn new_from_cli_matches(m: &'a clap::ArgMatches) -> Self {
        Naming {
            custom_image_name: m.value_of("IMAGE_NAME"),
            latest_tag: !m.is_present("NO_TAG_LATEST"),
            version_tag: !m.is_present("NO_TAG_VERSION"),
            version_release_tag: !m.is_present("NO_TAG_VERSION_RELEASE"),
            custom_tag: m.value_of("TAG_CUSTOM"),
        }
    }
}

pub fn export_for_cli_matches(ui: &mut UI, matches: &clap::ArgMatches) -> Result<()> {
    let default_channel = channel::default();
    let default_url = hurl::default_bldr_url();
    let spec = BuildSpec::new_from_cli_matches(&matches, &default_channel, &default_url);
    export(ui, spec)?;

    Ok(())
}

pub fn export(ui: &mut UI, build_spec: BuildSpec) -> Result<()> {

    let hart_to_package = build_spec.idents_or_archives.join(", ");
    let builder_url = build_spec.url;

    ui.begin(format!("Building a tarball with: {}", hart_to_package))?;

    let temp_dir_path = Temp::new_dir().unwrap().to_path_buf();

    initiate_tar_command(&temp_dir_path, &hart_to_package, &builder_url);

    Ok(())
}

fn initiate_tar_command(temp_dir_path: &PathBuf, hart_to_package: &str, builder_url: &str) {
    let status = Command::new("hab")
        .arg("studio")
        .arg("-r")
        .arg(&temp_dir_path)
        .arg("new")
        .status()
        .expect("failed to create studio");

    if status.success() {
        println!("Able to create studio to export package as tarball, proceeding...");
        install_command(&temp_dir_path, &hart_to_package, &builder_url);
    } else {
        println!("Unable to create a studio to export the package as a tarball.")
    }
}

fn install_command(temp_dir_path: &PathBuf, hart_to_package: &str, builder_url: &str) {
    let status = Command::new("hab")
        .arg("studio")
        .arg("-q")
        .arg("-r")
        .arg(&temp_dir_path)
        .arg("run")
        .arg("hab")
        .arg("install")
        .arg("-u")
        .arg(builder_url)
        .arg(&hart_to_package)
        .status()
        .expect("failed to install package in studio");

    if status.success() {
        println!("Hart package is installable in a studio, exporting tarball...");
        tar_command(&temp_dir_path, &hart_to_package);
    } else {
        println!("Hart package is NOT installable in a studio and could not be exported.");
        println!("Please see the above error for details.");
    }
}

fn tar_command(temp_dir_path: &PathBuf, hart_to_package: &str) {
    let status = Command::new("tar")
        .arg("cpzf")
        .arg(tar_name(&hart_to_package))
        .arg("-C")
        .arg(&temp_dir_path)
        .arg("./hab/pkgs")
        .arg("./hab/bin")
        .status()
        .expect("failed to create tarball");

    if status.success() {
        println!("Tarball export complete!")
    } else {
        println!("Unable to export package to tarball.")
    }
}

fn tar_name(hart_to_package: &str) -> String {
    let path = Path::new(hart_to_package);
    if path.is_file() {
        let ident = PackageArchive::new(path).ident().unwrap();
        format_tar_name(ident)
    } else {
        let pkg_path_command = Command::new("hab")
            .arg("pkg")
            .arg("path")
            .arg(hart_to_package)
            .output()
            .expect("Could not find path to habitat package to tar.");

        let pkg_path = String::from_utf8_lossy(&pkg_path_command.stdout).to_string();

        // Remove the trailing new line
        let edited_pkg_path = pkg_path.trim_matches('\n');

        // Get the identy portions of the path (origin/name/version/revision)
        let ident_regex = Regex::new(r"[\w-]+/([\w-]+/[\d/.]+/\d+$)").unwrap();
        let ident_captures = ident_regex.captures(edited_pkg_path).unwrap();
        let ident_string = &ident_captures[0];

        let ident = PackageIdent::from_str(ident_string).unwrap();
        format_tar_name(ident)
    }

}

fn format_tar_name(ident: PackageIdent) -> String {
    format!("{}-{}-{}-{}.tar.gz",
            ident.origin,
            ident.name,
            ident.version.unwrap(),
            ident.release.unwrap())
}
