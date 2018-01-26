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

use std::fs as stdfs;
#[cfg(target_os = "linux")]
use std::os::unix::fs::symlink;
use std::str::FromStr;
use clap;
use common;
use common::command::package::install::InstallSource;
use common::ui::{UI, Status};
use tempdir::TempDir;
use std::path::{Path, PathBuf};
use hcore::package::{PackageArchive, PackageIdent, PackageInstall};
use error::{Error, Result};
use hcore::PROGRAM_NAME;
use hcore::fs::{CACHE_ARTIFACT_PATH, CACHE_KEY_PATH, cache_artifact_path, cache_key_path};

use super::{VERSION, BUSYBOX_IDENT};

use rootfs;
use util;

const DEFAULT_HAB_IDENT: &'static str = "core/hab";
const DEFAULT_LAUNCHER_IDENT: &'static str = "core/hab-launcher";
const DEFAULT_SUP_IDENT: &'static str = "core/hab-sup";

/// The specification for creating a temporary file system build root, based on Habitat packages.
///
/// When a `BuildSpec` is created, a `BuildRoot` is returned which can be used to produce exported
/// images, archives, etc.
#[derive(Debug)]
pub struct BuildSpec<'a> {
    /// A string representation of a Habitat Package Identifer for the Habitat CLI package.
    pub hab: &'a str,
    /// A string representation of a Habitat Package Identifer for the Habitat Launcher package.
    pub hab_launcher: &'a str,
    /// A string representation of a Habitat Package Identifer for the Habitat Supervisor package.
    pub hab_sup: &'a str,
    /// The Builder URL which is used to install all service and extra Habitat packages.
    pub url: &'a str,
    /// The Habitat release channel which is used to install all service and extra Habitat
    /// packages.
    pub channel: &'a str,
    /// The Builder URL which is used to install all base Habitat packages.
    pub base_pkgs_url: &'a str,
    /// The Habitat release channel which is used to install all base Habitat packages.
    pub base_pkgs_channel: &'a str,
    /// A list of either Habitat Package Identifiers or local paths to Habitat Artifact files which
    /// will be installed.
    pub idents_or_archives: Vec<&'a str>,
}

impl<'a> BuildSpec<'a> {
    /// Creates a `BuildSpec` from cli arguments.
    pub fn new_from_cli_matches(
        m: &'a clap::ArgMatches,
        default_channel: &'a str,
        default_url: &'a str,
    ) -> Self {

        BuildSpec {
            hab: m.value_of("HAB_PKG").unwrap_or(DEFAULT_HAB_IDENT),
            hab_launcher: m.value_of("HAB_LAUNCHER_PKG").unwrap_or(
                DEFAULT_LAUNCHER_IDENT,
            ),
            hab_sup: m.value_of("HAB_SUP_PKG").unwrap_or(DEFAULT_SUP_IDENT),
            url: m.value_of("BLDR_URL").unwrap_or(&default_url),
            channel: m.value_of("CHANNEL").unwrap_or(&default_channel),
            base_pkgs_url: m.value_of("BASE_PKGS_BLDR_URL").unwrap_or(&default_url),
            base_pkgs_channel: m.value_of("BASE_PKGS_CHANNEL").unwrap_or(&default_channel),
            idents_or_archives: m.values_of("PKG_IDENT_OR_ARTIFACT")
                .expect("No package specified")
                .collect(),
        }
    }

    /// Creates a `BuildRoot` for the given specification.
    ///
    /// # Errors
    ///
    /// * If a temporary directory cannot be created
    /// * If the root file system cannot be created
    /// * If the `BuildRootContext` cannot be created
    pub fn create(self, ui: &mut UI) -> Result<BuildRoot> {
        let workdir = TempDir::new(&*PROGRAM_NAME)?;
        let rootfs = workdir.path().join("rootfs");

        ui.status(
            Status::Creating,
            format!("build root in {}", workdir.path().display()),
        )?;
        self.prepare_rootfs(ui, &rootfs)?;
println!("debugs to here");
        let ctx = BuildRootContext::from_spec(&self, rootfs)?;

        Ok(BuildRoot {
            workdir: workdir,
            ctx: ctx,
        })
    }

    fn prepare_rootfs<P: AsRef<Path>>(&self, ui: &mut UI, rootfs: P) -> Result<()> {
        ui.status(Status::Creating, "root filesystem")?;
        if cfg!(target_os = "linux") {
            rootfs::create(&rootfs)?;
        }
        self.create_symlink_to_artifact_cache(ui, &rootfs)?;
        self.create_symlink_to_key_cache(ui, &rootfs)?;
        let base_pkgs = self.install_base_pkgs(ui, &rootfs)?;
        let user_pkgs = self.install_user_pkgs(ui, &rootfs)?;
        self.remove_symlink_to_key_cache(ui, &rootfs)?;
        self.remove_symlink_to_artifact_cache(ui, &rootfs)?;

        Ok(())
    }

    fn create_symlink_to_artifact_cache<P: AsRef<Path>>(
        &self,
        ui: &mut UI,
        rootfs: P,
    ) -> Result<()> {
        ui.status(Status::Creating, "artifact cache symlink")?;
        let src = cache_artifact_path(None::<P>);
        let dst = rootfs.as_ref().join(CACHE_ARTIFACT_PATH);
        stdfs::create_dir_all(dst.parent().expect("parent directory exists"))?;
        debug!(
            "Symlinking src: {} to dst: {}",
            src.display(),
            dst.display()
        );

        Ok(symlink(src, dst)?)
    }

    fn create_symlink_to_key_cache<P: AsRef<Path>>(&self, ui: &mut UI, rootfs: P) -> Result<()> {
        ui.status(Status::Creating, "key cache symlink")?;
        let src = cache_key_path(None::<P>);
        let dst = rootfs.as_ref().join(CACHE_KEY_PATH);
        stdfs::create_dir_all(dst.parent().expect("parent directory exists"))?;
        debug!(
            "Symlinking src: {} to dst: {}",
            src.display(),
            dst.display()
        );

        Ok(symlink(src, dst)?)
    }

    fn install_base_pkgs<P: AsRef<Path>>(&self, ui: &mut UI, rootfs: P) -> Result<BasePkgIdents> {
        let hab = self.install_base_pkg(ui, self.hab, &rootfs)?;
        let sup = self.install_base_pkg(ui, self.hab_sup, &rootfs)?;
        let launcher = self.install_base_pkg(ui, self.hab_launcher, &rootfs)?;
        let busybox = if cfg!(target_os = "linux") {
            Some(self.install_base_pkg(ui, BUSYBOX_IDENT, &rootfs)?)
        } else {
            None
        };

        Ok(BasePkgIdents {
            hab,
            sup,
            launcher,
            busybox,
        })
    }

    fn install_user_pkgs<P: AsRef<Path>>(
        &self,
        ui: &mut UI,
        rootfs: P,
    ) -> Result<Vec<PackageIdent>> {
        let mut idents = Vec::new();
        for ioa in self.idents_or_archives.iter() {
            idents.push(self.install_user_pkg(ui, ioa, &rootfs)?);
        }

        Ok(idents)
    }

    fn install_base_pkg<P: AsRef<Path>>(
        &self,
        ui: &mut UI,
        ident_or_archive: &str,
        fs_root_path: P,
    ) -> Result<PackageIdent> {
        self.install(
            ui,
            ident_or_archive,
            self.base_pkgs_url,
            self.base_pkgs_channel,
            fs_root_path,
        )
    }

    fn install_user_pkg<P: AsRef<Path>>(
        &self,
        ui: &mut UI,
        ident_or_archive: &str,
        fs_root_path: P,
    ) -> Result<PackageIdent> {
        self.install(ui, ident_or_archive, self.url, self.channel, fs_root_path)
    }

    fn install<P: AsRef<Path>>(
        &self,
        ui: &mut UI,
        ident_or_archive: &str,
        url: &str,
        channel: &str,
        fs_root_path: P,
    ) -> Result<PackageIdent> {

        let install_source: InstallSource = ident_or_archive.parse()?;
        let package_install = common::command::package::install::start(
            ui,
            url,
            Some(channel),
            &install_source,
            &*PROGRAM_NAME,
            VERSION,
            &fs_root_path,
            &cache_artifact_path(Some(&fs_root_path)),
            None,
        )?;
        Ok(package_install.into())
    }

    fn remove_symlink_to_artifact_cache<P: AsRef<Path>>(
        &self,
        ui: &mut UI,
        rootfs: P,
    ) -> Result<()> {
        ui.status(Status::Deleting, "artifact cache symlink")?;
        stdfs::remove_dir_all(rootfs.as_ref().join(CACHE_ARTIFACT_PATH))?;
        Ok(())
    }

    fn remove_symlink_to_key_cache<P: AsRef<Path>>(&self, ui: &mut UI, rootfs: P) -> Result<()> {
        ui.status(Status::Deleting, "artifact key symlink")?;
        stdfs::remove_dir_all(rootfs.as_ref().join(CACHE_KEY_PATH))?;

        Ok(())
    }

}

#[derive(Debug)]
pub struct BuildRoot {
    /// The temporary directory under which all root file system and other related files and
    /// directories will be created.
    workdir: TempDir,
    /// The build root context containing information about Habitat packages, `PATH` info, etc.
    ctx: BuildRootContext,
}

/// The file system contents, location, Habitat pacakges, and other context for a build root.
#[derive(Debug)]
pub struct BuildRootContext {
    /// A list of all Habitat service and library packages which were determined from the original
    /// list in a `BuildSpec`.
    idents: Vec<PkgIdentType>,
    /// The `bin` path which will be used for all program symlinking.
    bin_path: PathBuf,
    /// A string representation of the build root's `PATH` environment variable value (i.e. a
    /// colon-delimited `PATH` string).
    env_path: String,
    /// The channel name which was used to install all user-provided Habitat service and library
    /// packages.
    channel: String,
    /// The path to the root of the file system.
    rootfs: PathBuf,
}

impl BuildRootContext {
    /// Creates a new `BuildRootContext` from a build spec.
    ///
    /// The root file system path will be used to inspect installed Habitat packages to populate
    /// metadata, determine primary service, etc.
    ///
    /// # Errors
    ///
    /// * If an artifact file cannot be read or if a Package Identifier cannot be determined
    /// * If a Package Identifier cannot be parsed from an string representation
    /// * If package metadata cannot be read
    pub fn from_spec<P: Into<PathBuf>>(spec: &BuildSpec, rootfs: P) -> Result<Self> {
println!("one");
        let rootfs = rootfs.into();
println!("two");
        let mut idents = Vec::new();
println!("three");
        for ident_or_archive in &spec.idents_or_archives {
            let ident = if Path::new(ident_or_archive).is_file() {
                // We're going to use the `$pkg_origin/$pkg_name`, fuzzy form of a package
                // identifier to ensure that update strategies will work if desired
                let mut archive_ident = PackageArchive::new(ident_or_archive).ident()?;
                archive_ident.version = None;
                archive_ident.release = None;
                archive_ident
            } else {
                PackageIdent::from_str(ident_or_archive)?
            };
println!("four");
            let pkg_install = PackageInstall::load(&ident, Some(&rootfs))?;
println!("five");
            if pkg_install.is_runnable() {
                idents.push(PkgIdentType::Svc(SvcIdent {
                    ident: ident,
                    exposes: pkg_install.exposes()?,
                }));
            } else {
                idents.push(PkgIdentType::Lib(ident));
            }
        }
        let bin_path = util::bin_path();

        let context = BuildRootContext {
            idents: idents,
            bin_path: bin_path.into(),
            env_path: bin_path.to_string_lossy().into_owned(),
            channel: spec.channel.into(),
            rootfs: rootfs,
        };
        context.validate()?;

        Ok(context)
    }

    fn validate(&self) -> Result<()> {
        // A valid context for a build root will contain at least one service package, called the
        // primary service package.
        if let None = self.svc_idents().first().map(|e| *e) {
            return Err(Error::PrimaryServicePackageNotFound(
                self.idents.iter().map(|e| e.ident().to_string()).collect(),
            ))?;
        }

        Ok(())
    }

    /// Returns a list of all provided Habitat packages which contain a runnable service.
    pub fn svc_idents(&self) -> Vec<&PackageIdent> {
        self.idents
            .iter()
            .filter_map(|t| match *t {
                PkgIdentType::Svc(ref svc) => Some(svc.ident.as_ref()),
                _ => None,
            })
            .collect()
    }
}

/// A service identifier representing a Habitat package which contains a runnable service.
#[derive(Debug)]
struct SvcIdent {
    /// The Package Identifier.
    pub ident: PackageIdent,
    /// A list of all port exposes for the package.
    pub exposes: Vec<String>,
}


/// An enum of service and library Habitat packages.
///
/// A package is considered a service package if it contains a runnable service, via a `run` hook.
#[derive(Debug)]
enum PkgIdentType {
    /// A service package which contains a runnable service.
    Svc(SvcIdent),
    /// A library package which does not contain a runnable service.
    Lib(PackageIdent),
}

impl PkgIdentType {
    /// Returns the Package Identifier for the package type.
    pub fn ident(&self) -> &PackageIdent {
        match *self {
            PkgIdentType::Svc(ref svc) => &svc.ident,
            PkgIdentType::Lib(ref ident) => &ident,
        }
    }
}

/// The package identifiers for installed base packages.
#[derive(Debug)]
struct BasePkgIdents {
    /// Installed package identifer for the Habitat CLI package.
    pub hab: PackageIdent,
    /// Installed package identifer for the Supervisor package.
    pub sup: PackageIdent,
    /// Installed package identifer for the Launcher package.
    pub launcher: PackageIdent,
    /// Installed package identifer for the Busybox package.
    pub busybox: Option<PackageIdent>,
}
