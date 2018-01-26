use common::ui::{UI};
use build::BuildRoot;
use error::{Error, Result};

/// A temporary file system build root for building a tarball, based on Habitat packages.
#[derive(Debug)]
pub struct TarBuildRoot(BuildRoot);

impl TarBuildRoot {
    pub fn from_build_root(build_root: BuildRoot, ui: &mut UI) -> Result<Self> {
        let root = TarBuildRoot(build_root);
        println!("here is the root {:?}", root);
        Ok(root)
    }
}
