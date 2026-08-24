use std::ffi::{CStr, CString, c_void};
use std::marker::PhantomData;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::rc::Rc;

use ghostty_sys::ffi;
use thiserror::Error;

use crate::runtime::{
    InitializationError, MainThreadError, initialize_ghostty, require_main_thread,
};

const MUXY_USER_CONFIG: &str = "Library/Application Support/Muxy/ghostty.conf";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ColorScheme {
    Light,
    Dark,
}

impl ColorScheme {
    pub(crate) const fn as_raw(self) -> ffi::ghostty_color_scheme_e {
        match self {
            Self::Light => ffi::ghostty_color_scheme_e_GHOSTTY_COLOR_SCHEME_LIGHT,
            Self::Dark => ffi::ghostty_color_scheme_e_GHOSTTY_COLOR_SCHEME_DARK,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigPaths {
    bundled_defaults: PathBuf,
    generated_overlay: Option<PathBuf>,
    user_override: Option<PathBuf>,
}

impl ConfigPaths {
    pub fn new(bundled_defaults: impl Into<PathBuf>) -> Self {
        Self::with_home(
            bundled_defaults,
            std::env::var_os("HOME").as_deref().map(Path::new),
        )
    }

    pub fn with_home(bundled_defaults: impl Into<PathBuf>, home: Option<&Path>) -> Self {
        Self {
            bundled_defaults: bundled_defaults.into(),
            generated_overlay: None,
            user_override: home.map(|home| home.join(MUXY_USER_CONFIG)),
        }
    }

    pub fn with_generated_overlay(mut self, generated_overlay: Option<PathBuf>) -> Self {
        self.generated_overlay = generated_overlay;
        self
    }

    pub fn with_user_override(mut self, user_override: Option<PathBuf>) -> Self {
        self.user_override = user_override;
        self
    }

    pub fn bundled_defaults(&self) -> &Path {
        &self.bundled_defaults
    }

    pub fn generated_overlay(&self) -> Option<&Path> {
        self.generated_overlay.as_deref()
    }

    pub fn user_override(&self) -> Option<&Path> {
        self.user_override.as_deref()
    }

    fn load_order_with(&self, exists: impl Fn(&Path) -> bool) -> Vec<ConfigSource> {
        let mut sources = vec![
            ConfigSource::DefaultFiles,
            ConfigSource::File(self.bundled_defaults.clone()),
        ];
        if let Some(generated_overlay) = self.generated_overlay.as_ref() {
            sources.push(ConfigSource::File(generated_overlay.clone()));
        }
        if let Some(user_override) = self.user_override.as_ref().filter(|path| exists(path)) {
            sources.push(ConfigSource::File(user_override.clone()));
        }
        sources
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ConfigSource {
    DefaultFiles,
    File(PathBuf),
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ConfigError {
    #[error(transparent)]
    Initialization(#[from] InitializationError),
    #[error(transparent)]
    MainThread(#[from] MainThreadError),
    #[error("ghostty_config_new returned null")]
    CreationReturnedNull,
    #[error("ghostty_config_clone returned null")]
    CloneReturnedNull,
    #[error("Ghostty config path contains an interior NUL byte: {path}")]
    PathContainsNul { path: PathBuf },
    #[error("Ghostty config diagnostic {index} returned a null message")]
    DiagnosticMessageReturnedNull { index: u32 },
}

pub struct GhosttyConfig {
    raw: NonNull<c_void>,
    diagnostics: Vec<String>,
    _main_thread_only: PhantomData<Rc<()>>,
}

impl GhosttyConfig {
    pub fn load(paths: ConfigPaths) -> Result<Self, ConfigError> {
        require_main_thread()?;
        initialize_ghostty()?;

        let raw = unsafe { ffi::ghostty_config_new() };
        let raw = NonNull::new(raw).ok_or(ConfigError::CreationReturnedNull)?;
        let mut config = Self {
            raw,
            diagnostics: Vec::new(),
            _main_thread_only: PhantomData,
        };

        for source in paths.load_order_with(Path::is_file) {
            match source {
                ConfigSource::DefaultFiles => {
                    unsafe { ffi::ghostty_config_load_default_files(config.as_raw()) };
                }
                ConfigSource::File(path) => {
                    let c_path = path_to_cstring(&path)?;
                    unsafe { ffi::ghostty_config_load_file(config.as_raw(), c_path.as_ptr()) };
                }
            }
        }

        unsafe { ffi::ghostty_config_finalize(config.as_raw()) };
        config.diagnostics = config.copy_diagnostics()?;
        Ok(config)
    }

    pub fn try_clone(&self) -> Result<Self, ConfigError> {
        require_main_thread()?;
        let raw = unsafe { ffi::ghostty_config_clone(self.as_raw()) };
        let raw = NonNull::new(raw).ok_or(ConfigError::CloneReturnedNull)?;
        let mut config = Self {
            raw,
            diagnostics: Vec::new(),
            _main_thread_only: PhantomData,
        };
        config.diagnostics = config.copy_diagnostics()?;
        Ok(config)
    }

    pub fn with_overlay_file(&self, path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        require_main_thread()?;
        let path = path.as_ref();
        let c_path = path_to_cstring(path)?;
        let mut overlay = self.try_clone()?;
        unsafe { ffi::ghostty_config_load_file(overlay.as_raw(), c_path.as_ptr()) };
        overlay.diagnostics = overlay.copy_diagnostics()?;
        Ok(overlay)
    }

    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    pub(crate) fn as_raw(&self) -> ffi::ghostty_config_t {
        self.raw.as_ptr()
    }

    fn copy_diagnostics(&self) -> Result<Vec<String>, ConfigError> {
        let count = unsafe { ffi::ghostty_config_diagnostics_count(self.as_raw()) };
        let mut diagnostics = Vec::with_capacity(count as usize);
        for index in 0..count {
            let diagnostic = unsafe { ffi::ghostty_config_get_diagnostic(self.as_raw(), index) };
            if diagnostic.message.is_null() {
                return Err(ConfigError::DiagnosticMessageReturnedNull { index });
            }
            let message = unsafe { CStr::from_ptr(diagnostic.message) }
                .to_string_lossy()
                .into_owned();
            diagnostics.push(message);
        }
        Ok(diagnostics)
    }
}

impl Drop for GhosttyConfig {
    fn drop(&mut self) {
        unsafe { ffi::ghostty_config_free(self.as_raw()) };
    }
}

fn path_to_cstring(path: &Path) -> Result<CString, ConfigError> {
    CString::new(path.as_os_str().as_bytes()).map_err(|_| ConfigError::PathContainsNul {
        path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_path_order_is_defaults_then_bundle_then_existing_user_override() {
        let home = Path::new("/Users/tester");
        let paths =
            ConfigPaths::with_home("/Muxy.app/Contents/Resources/muxy-defaults", Some(home));
        let user = home.join(MUXY_USER_CONFIG);
        let order = paths.load_order_with(|path| path == user);

        assert_eq!(
            order,
            vec![
                ConfigSource::DefaultFiles,
                ConfigSource::File(PathBuf::from("/Muxy.app/Contents/Resources/muxy-defaults")),
                ConfigSource::File(user),
            ]
        );
    }

    #[test]
    fn absent_optional_user_config_is_skipped_without_reordering_required_sources() {
        let paths = ConfigPaths::with_home("/bundle/muxy-defaults", Some(Path::new("/home")));
        assert_eq!(
            paths.load_order_with(|_| false),
            vec![
                ConfigSource::DefaultFiles,
                ConfigSource::File(PathBuf::from("/bundle/muxy-defaults")),
            ]
        );
    }

    #[test]
    fn generated_overlay_precedes_user_override() {
        let paths = ConfigPaths::with_home("/bundle/defaults", Some(Path::new("/home")))
            .with_generated_overlay(Some(PathBuf::from("/tmp/cjk.conf")));
        assert_eq!(
            paths.load_order_with(|_| true),
            vec![
                ConfigSource::DefaultFiles,
                ConfigSource::File(PathBuf::from("/bundle/defaults")),
                ConfigSource::File(PathBuf::from("/tmp/cjk.conf")),
                ConfigSource::File(PathBuf::from(
                    "/home/Library/Application Support/Muxy/ghostty.conf"
                )),
            ]
        );
    }
}
