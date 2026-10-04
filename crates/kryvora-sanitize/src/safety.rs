// file: crates/kryvora-sanitize/src/safety.rs
//! System-path protection.
//!
//! Protects the directories that hold the operating system and
//! installed applications. Does **not** protect the user's own data
//! directories, including `%APPDATA%` and `%LOCALAPPDATA%`, because
//! those contain user data and are legitimate sanitization targets.

use std::path::{Path, PathBuf};

/// Why a path is protected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SystemPathReason {
    WindowsSystemRoot { root: String },
    ProgramFiles { root: String },
    ProgramData { root: String },
}

impl SystemPathReason {
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::WindowsSystemRoot { root } => {
                format!("target is inside the Windows system root: {root}")
            }
            Self::ProgramFiles { root } => {
                format!("target is inside Program Files: {root}")
            }
            Self::ProgramData { root } => {
                format!("target is inside ProgramData: {root}")
            }
        }
    }

    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::WindowsSystemRoot { .. } => "system_root",
            Self::ProgramFiles { .. } => "program_files",
            Self::ProgramData { .. } => "program_data",
        }
    }
}

/// The shape of a protected-root entry: the environment variable name
/// and a constructor that builds a [`SystemPathReason`] from its
/// canonical value.
type SystemPathCheck = (&'static str, fn(&Path) -> SystemPathReason);

/// Return the reason `path` is a protected system path, or `None` if
/// it is not.
#[must_use]
pub fn is_system_path(path: &Path) -> Option<SystemPathReason> {
    let canon = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let canon_norm = normalize_case(&canon);

    for (var, ctor) in PROTECTED_ENV_VARS {
        let Ok(root) = std::env::var(var) else {
            continue;
        };
        if root.is_empty() {
            continue;
        }
        let root_path = PathBuf::from(&root);
        let root_canon = std::fs::canonicalize(&root_path).unwrap_or(root_path);
        let root_norm = normalize_case(&root_canon);

        if canon_norm == root_norm || canon_norm.starts_with(&root_norm) {
            return Some(ctor(&root_canon));
        }
    }

    None
}

const PROTECTED_ENV_VARS: &[SystemPathCheck] = &[
    ("SystemRoot", |p| SystemPathReason::WindowsSystemRoot {
        root: p.display().to_string(),
    }),
    ("windir", |p| SystemPathReason::WindowsSystemRoot {
        root: p.display().to_string(),
    }),
    ("ProgramFiles", |p| SystemPathReason::ProgramFiles {
        root: p.display().to_string(),
    }),
    ("ProgramFiles(x86)", |p| SystemPathReason::ProgramFiles {
        root: p.display().to_string(),
    }),
    ("ProgramData", |p| SystemPathReason::ProgramData {
        root: p.display().to_string(),
    }),
];

#[cfg(windows)]
fn normalize_case(p: &Path) -> PathBuf {
    use std::path::Component;
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Prefix(pre) => {
                let lower = pre.as_os_str().to_string_lossy().to_lowercase();
                out.push(lower);
            }
            Component::RootDir => out.push(Component::RootDir.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => out.push(".."),
            Component::Normal(s) => {
                let lower = s.to_string_lossy().to_lowercase();
                out.push(lower);
            }
        }
    }
    out
}

#[cfg(not(windows))]
fn normalize_case(p: &Path) -> PathBuf {
    p.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_paths_are_not_protected() {
        let p = std::env::temp_dir().join("kryvora-safety-test-ordinary");
        assert!(is_system_path(&p).is_none());
    }

    #[test]
    fn appdata_is_not_protected() {
        // The user's own data directory is not a protected system
        // path. This is deliberate: users must be able to sanitize
        // their own files.
        if let Ok(root) = std::env::var("LOCALAPPDATA") {
            let probe = PathBuf::from(&root).join("some-user-data");
            assert!(is_system_path(&probe).is_none());
        }
    }

    #[test]
    fn does_not_panic_on_arbitrary_paths() {
        let _ = is_system_path(Path::new("C:/some/ordinary/path"));
        let _ = is_system_path(Path::new(""));
    }

    #[test]
    fn message_and_code_are_non_empty() {
        for r in [
            SystemPathReason::WindowsSystemRoot { root: "x".into() },
            SystemPathReason::ProgramFiles { root: "x".into() },
            SystemPathReason::ProgramData { root: "x".into() },
        ] {
            assert!(!r.message().is_empty());
            assert!(!r.code().is_empty());
        }
    }
}
