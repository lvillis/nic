#![allow(clippy::needless_collect)]
//! Build script
//!
//! * Embeds an application manifest requesting admin privileges
//! * Embeds an ICO so the Windows executable has a proper icon

use std::{env, fs, path::PathBuf};

const MANIFEST: &str = r#"
<?xml version="1.0" encoding="utf-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#;

fn main() {
    #[cfg(windows)]
    {
        // ----- generate manifest -----
        let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
        let manifest_path = out_dir.join("nic_manifest.xml");
        fs::write(&manifest_path, MANIFEST).expect("failed to write manifest");

        // ----- generate .rc -----
        // 1  ICON     "icon.ico"
        // 24 MANIFEST manifest_path
        let rc_path = out_dir.join("nic.rc");
        let manifest_str = manifest_path.to_string_lossy().replace('\\', "/");
        fs::write(
            &rc_path,
            format!(
                "1 ICON \"{ico}\"\n1 24 \"{man}\"",
                ico = project_icon().replace('\\', "/"),
                man = manifest_str
            ),
        )
            .expect("failed to write resource file");

        // ----- compile resources -----
        embed_resource::compile(&rc_path, embed_resource::NONE)
            .manifest_required()
            .unwrap();
    }
}

/// Absolute path to `logo.ico` in project root.  
/// Panics if the file does not exist.
#[cfg(windows)]
fn project_icon() -> String {
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let ico = crate_dir.join("logo.ico");
    if !ico.exists() {
        panic!("logo.ico missing in repository root");
    }
    ico.to_string_lossy().into_owned()
}
