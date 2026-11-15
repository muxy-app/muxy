use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Build<'a> {
    pub target: &'a str,
    pub host: &'a str,
    pub optimize: &'a str,
    pub cpu: &'a str,
}

pub struct KeptInstall {
    directory: PathBuf,
    slot: String,
}

impl KeptInstall {
    pub fn find(static_link: bool, build: &Build) -> Option<Self> {
        let root = env::var_os("LIBGHOSTTY_VT_SYS_ZIG_CACHE_DIR").filter(|dir| !dir.is_empty())?;
        if !static_link
            || env::var_os("GHOSTTY_SOURCE_DIR").is_some()
            || env::var_os("GHOSTTY_ZIG_SYSTEM_DIR").is_some()
        {
            return None;
        }
        let zig = env::var_os("ZIG").unwrap_or_else(|| "zig".into());
        let version = Command::new(zig)
            .arg("version")
            .output()
            .ok()
            .filter(|output| output.status.success())?
            .stdout;
        let mut key = Key::default();
        for part in [
            crate::GHOSTTY_COMMIT.as_bytes(),
            include_bytes!("build.rs"),
            include_bytes!("patch.rs"),
            include_bytes!("kept_install.rs"),
            include_bytes!("patches/clear-screen.patch"),
            include_bytes!("patches/kitty-image-file.patch"),
            include_bytes!("patches/kitty-animation-frames.patch"),
            &version,
            build.target.as_bytes(),
            build.host.as_bytes(),
            build.optimize.as_bytes(),
            build.cpu.as_bytes(),
        ] {
            key.add(part);
        }
        for name in ["MACOSX_DEPLOYMENT_TARGET", "IPHONEOS_DEPLOYMENT_TARGET", "SDKROOT"] {
            key.add(env::var_os(name).unwrap_or_default().as_encoded_bytes());
        }
        let slot = format!("{}-{}-{}", build.target, build.cpu, build.optimize);
        Some(Self {
            directory: Path::new(&root)
                .join("libghostty-vt")
                .join(format!("{slot}-{:016x}", key.0)),
            slot,
        })
    }

    pub fn restore(&self, prefix: &Path) -> bool {
        if !self.directory.join("include/ghostty/vt.h").is_file() {
            return false;
        }
        let _ = fs::remove_dir_all(prefix);
        match copy_files(&self.directory, prefix) {
            Ok(()) => true,
            Err(error) => {
                println!(
                    "cargo:warning=rebuilding Ghostty: could not restore {}: {error}",
                    self.directory.display()
                );
                let _ = fs::remove_dir_all(prefix);
                false
            }
        }
    }

    pub fn keep(&self, prefix: &Path) {
        let Some(parent) = self.directory.parent() else {
            return;
        };
        let partial = parent.join(format!(".partial-{}", std::process::id()));
        let _ = fs::remove_dir_all(&partial);
        let kept = copy_files(prefix, &partial).and_then(|()| {
            for entry in fs::read_dir(parent)? {
                let entry = entry?;
                if self.is_older(&entry.file_name().to_string_lossy()) {
                    fs::remove_dir_all(entry.path())?;
                }
            }
            fs::rename(&partial, &self.directory)
        });
        if let Err(error) = kept {
            let _ = fs::remove_dir_all(&partial);
            if !self.directory.exists() {
                println!(
                    "cargo:warning=could not keep the Ghostty build in {}: {error}",
                    self.directory.display()
                );
            }
        }
    }

    fn is_older(&self, name: &str) -> bool {
        let current = self.directory.file_name().map(|name| name.to_string_lossy());
        name.strip_prefix(&self.slot)
            .and_then(|rest| rest.strip_prefix('-'))
            .is_some_and(|hash| hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
            && current.as_deref() != Some(name)
    }
}

fn copy_files(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if file_type.is_dir() {
            copy_files(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

struct Key(u64);

impl Default for Key {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}

impl Key {
    fn add(&mut self, part: &[u8]) {
        for byte in (part.len() as u64).to_le_bytes().iter().chain(part) {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}
