//! A crate for resolving relative (`./`) and tilde paths (`~/`) in Rust.
//!
//! Note that this does not perform _path canonicalization_, i.e. it will
//! not eliminate segments like `..` or `./././` in a path. This crate
//! is intended simply to anchor relative paths such that they have an
//! absolute path from the root.
//!
//! # Motivation
//!
//! Rust has `Path` and `PathBuf` in the standard library for working with
//! file paths, but unfortunately there is no easy and ergonomic way to
//! resolve relative paths in the following ways:
//!
//! - with respect to the process current-working-directory (CWD)
//! - with respect to the active user's home directory (`~/`)
//! - with respect to a user-provided absolute path
//!
//! # API
//!
//! This crate provides an extension trait [`PathResolveExt`] with extension
//! methods for path-like types. The following methods are provided:
//!
//! ## `resolve` and `try_resolve`
//!
//! These methods will resolve relative paths (`./...`) with respect to the
//! process current-working-directory, and will also resolve tilde-paths (`~/...`)
//! to the active user's home directory.
//!
//! Assuming a home directory of `/home/user` and a CWD of `/home/user/Documents`,
//! the `resolve` methods will evaluate in the following ways:
//!
//! ```no_run
//! use std::path::Path;
//! use resolve_path::PathResolveExt;
//!
//! // Direct variant (may panic)
//! assert_eq!("~/.vimrc".resolve(), Path::new("/home/user/.vimrc"));
//! assert_eq!("./notes.txt".resolve(), Path::new("/home/user/Documents/notes.txt"));
//!
//! // Try variant (returns Result)
//! assert_eq!("~/.vimrc".try_resolve().unwrap(), Path::new("/home/user/.vimrc"));
//! assert_eq!("./notes.txt".try_resolve().unwrap(), Path::new("/home/user/Documents/notes.txt"));
//! ```
//!
//! ## `resolve_in` and `try_resolve_in`
//!
//! These methods will resolve tilde-paths (`~/...`) in the normal way, but will
//! resolve relative paths (`./...`) with respect to a provided base directory.
//! This can be very useful, for example when evaluating paths given in a config
//! file with respect to the location of the config file, rather than with respect
//! to the process CWD.
//!
//! Assuming the same home directory of `/home/user` and CWD of `/home/user/Documents`,
//! the `resolve_in` methods will evaluate in the following ways:
//!
//! ```no_run
//! use std::path::Path;
//! use resolve_path::PathResolveExt;
//!
//! // Direct variant (may panic)
//! assert_eq!("~/.vimrc".resolve_in("~/.config/alacritty/"), Path::new("/home/user/.vimrc"));
//! assert_eq!("./alacritty.yml".resolve_in("~/.config/alacritty/"), Path::new("/home/user/.config/alacritty/alacritty.yml"));
//!
//! // Try variant (returns Result)
//! assert_eq!("~/.vimrc".try_resolve_in("~/.config/alacritty/").unwrap(), Path::new("/home/user/.vimrc"));
//! assert_eq!("./alacritty.yml".try_resolve_in("~/.config/alacritty/").unwrap(), Path::new("/home/user/.config/alacritty/alacritty.yml"));
//! ```
//!
//! ## Why use `Cow<Path>`?
//!
//! If any of the [`PathResolveExt`] methods are called on a path that does not
//! actually need to be resolved (i.e. a path that is already absolute), then
//! the resolver methods will simply return `Cow::Borrowed(&Path)` with the original
//! path ref within. If resolution _does_ occur, then the path will one way or another
//! be edited (e.g. by adding an absolute path prefix), and will be returned as
//! a `Cow::Owned(PathBuf)`. This way we can avoid allocation where it is unnecessary.

use std::borrow::Cow;
use std::ffi::OsStr;
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};

type Result<T, E = IoError> = core::result::Result<T, E>;

/// Extension trait for resolving paths against a base path.
///
/// # Example
///
/// ```
/// use std::path::Path;
/// use resolve_path::PathResolveExt as _;
/// assert_eq!(Path::new("./config.yml").resolve_in("/home/user/.app"), Path::new("/home/user/.app/config.yml"));
/// ```
pub trait PathResolveExt {
    /// Resolves the path in the process's current directory
    ///
    /// # Example
    ///
    /// ```no_run
    /// use std::path::Path;
    /// use resolve_path::PathResolveExt;
    /// std::env::set_current_dir("/home/user/.config/alacritty").unwrap();
    /// let resolved = "./alacritty.yml".resolve();
    /// assert_eq!(resolved, Path::new("/home/user/.config/alacritty"));
    /// ```
    ///
    /// # Panics
    ///
    /// This function panics if:
    ///
    /// - It is unable to detect the current working directory
    /// - It is unable to resolve the home directory for a tilde (`~`)
    ///
    /// See [`try_resolve`][`PathResolveExt::try_resolve`] for a non-panicking API.
    fn resolve(&self) -> Cow<Path> {
        self.try_resolve()
            .expect("should resolve path in current directory")
    }

    /// Attempts to resolve the path in the process's current directory
    ///
    /// Returns an error if:
    ///
    /// - It is unable to detect the current working directory
    /// - It is unable to resolve the home directory for a tilde (`~`)
    fn try_resolve(&self) -> Result<Cow<Path>> {
        let cwd = std::env::current_dir()?;
        let resolved = self.try_resolve_in(&cwd)?;
        Ok(resolved)
    }

    /// Resolves this path against a given base path.
    ///
    /// # Example
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use resolve_path::PathResolveExt as _;
    ///
    /// assert_eq!("./config.yml".resolve_in("/home/user/.app"), Path::new("/home/user/.app/config.yml"));
    /// assert_eq!(String::from("./config.yml").resolve_in("/home/user/.app"), Path::new("/home/user/.app/config.yml"));
    /// assert_eq!(Path::new("./config.yml").resolve_in("/home/user/.app"), Path::new("/home/user/.app/config.yml"));
    /// assert_eq!(PathBuf::from("./config.yml").resolve_in("/home/user/.app"), Path::new("/home/user/.app/config.yml"));
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if we attempt to resolve a `~` in either path and are
    /// unable to determine the home directory from the environment
    /// (using the `dirs` crate). See [`try_resolve_in`][`PathResolveExt::try_resolve_in`]
    /// for a non-panicking option.
    fn resolve_in<P: AsRef<Path>>(&self, base: P) -> Cow<Path> {
        self.try_resolve_in(base).expect("should resolve path")
    }

    /// Resolves this path against a given base path, returning an error
    /// if unable to resolve a home directory.
    fn try_resolve_in<P: AsRef<Path>>(&self, base: P) -> Result<Cow<Path>>;
}

impl<T: AsRef<OsStr>> PathResolveExt for T {
    fn try_resolve_in<P: AsRef<Path>>(&self, base: P) -> Result<Cow<Path>> {
        try_resolve_path(base.as_ref(), Path::new(self))
    }
}

fn try_resolve_path<'a>(base: &Path, to_resolve: &'a Path) -> Result<Cow<'a, Path>> {
    // If the path to resolve is absolute, there's no relativity to resolve
    if to_resolve.is_absolute() {
        return Ok(Cow::Borrowed(to_resolve));
    }

    // If the path to resolve has a tilde, resolve it to home and be done
    if to_resolve.starts_with(Path::new("~")) {
        let resolved = resolve_tilde(to_resolve)?;
        return Ok(resolved);
    }

    // Resolve the base path by expanding tilde if needed
    let absolute_base = if base.is_absolute() {
        base.to_owned()
    } else {
        // Attempt to resolve a tilde in the base path
        let base_resolved_tilde = resolve_tilde(base)?;
        if base_resolved_tilde.is_relative() {
            return Err(IoError::new(
                ErrorKind::InvalidData,
                "the base path must be able to resolve to an absolute path",
            ));
        }

        base_resolved_tilde.into_owned()
    };

    // If the base path points to a file, use that file's parent directory as the base
    let base_directory = match std::fs::metadata(&absolute_base) {
        Ok(meta) => {
            // If we know this path points to a file, use the file's parent dir
            if meta.is_file() {
                match absolute_base.parent() {
                    Some(parent) => parent.to_path_buf(),
                    None => {
                        return Err(IoError::new(
                            ErrorKind::NotFound,
                            "the base path points to a file with no parent directory",
                        ))
                    }
                }
            } else {
                // If we know this path points to a dir, use it
                absolute_base
            }
        }
        // If we cannot get FS metadata about this path, just use it as-is
        Err(_) => absolute_base,
    };

    let resolved = base_directory.join(to_resolve);
    Ok(Cow::Owned(resolved))
}

/// Resolve a tilde in the given path to the home directory, if a tilde is present.
///
/// - If the path does not begin with a tilde, returns the original path
/// - If the path is not valid UTF-8, returns the original path
/// - If the tilde names another user (e.g. `~user`), returns the original path
/// - Otherwise, resolves the tilde to the homedir and joins with the remaining path
///
/// # Example
///
/// ```ignore
/// # use std::path::Path;
/// # use resolve_path::resolve_tilde;
/// assert_eq!(resolve_tilde(Path::new("~")).unwrap(), Path::new("/home/test"));
/// assert_eq!(resolve_tilde(Path::new("~/.config")).unwrap(), Path::new("/home/test/.config"));
/// assert_eq!(resolve_tilde(Path::new("/tmp/hello")).unwrap(), Path::new("/tmp/hello"));
/// assert_eq!(resolve_tilde(Path::new("./configure")).unwrap(), Path::new("./configure"));
/// ```
fn resolve_tilde(path: &Path) -> Result<Cow<Path>> {
    let home = home_dir().ok_or_else(|| IoError::new(ErrorKind::NotFound, "homedir not found"))?;
    Ok(resolve_tilde_with_home(home, path))
}

/// Resolve a tilde in a given path to a _given_ home directory.
///
/// - If the path does not begin with a tilde, returns the original path
/// - If the path is not valid UTF-8, returns the original path
/// - If the tilde names another user (e.g. `~user`), returns the original path
/// - Otherwise, resolves the tilde to the homedir and joins with the remaining path
///
/// # Example
///
/// ```ignore
/// # use std::path::{Path, PathBuf};
/// # use resolve_path::resolve_tilde_with_home;
/// assert_eq!(resolve_tilde_with_home(PathBuf::from("/home/test"), Path::new("~")), Path::new("/home/test"));
/// assert_eq!(resolve_tilde_with_home(PathBuf::from("/home/test"), Path::new("~/.config")), Path::new("/home/test/.config"));
/// assert_eq!(resolve_tilde_with_home(PathBuf::from("/home/test"), Path::new("/tmp/hello")), Path::new("/tmp/hello"));
/// assert_eq!(resolve_tilde_with_home(PathBuf::from("/home/test"), Path::new("./configure")), Path::new("./configure"));
/// ```
fn resolve_tilde_with_home(home: PathBuf, path: &Path) -> Cow<Path> {
    // If this path has no tilde, return it as-is
    if !path.starts_with(Path::new("~")) {
        return Cow::Borrowed(path);
    }

    // If we have a tilde, strip it and convert the remainder to UTF-8 str slice
    let path_str = match path.to_str() {
        Some(s) => s,
        None => return Cow::Borrowed(path),
    };
    let stripped = &path_str[1..];

    // Support a solo "~" with no trailing path
    if stripped.is_empty() {
        return Cow::Owned(home);
    }

    // Support a path starting with "~/..."
    if stripped.starts_with('/') {
        let stripped = stripped.trim_start_matches('/');
        let resolved = home.join(stripped);
        return Cow::Owned(resolved);
    }

    // If we have something like "~user", return original path
    Cow::Borrowed(path)
}

#[allow(unused)]
#[cfg(not(test))]
fn home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

/// During testing, always resolve home to /home/test
#[allow(unused)]
#[cfg(test)]
fn home_dir() -> Option<PathBuf> {
    Some(PathBuf::from("/home/test"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn test_resolve_tilde() {
        assert_eq!("~".resolve(), Path::new("/home/test"));
        assert_eq!("~".to_string().resolve(), Path::new("/home/test"));
        assert_eq!(Path::new("~").resolve(), Path::new("/home/test"));
        assert_eq!(PathBuf::from("~").resolve(), Path::new("/home/test"));
        assert_eq!(OsStr::new("~").resolve(), Path::new("/home/test"));
        assert_eq!(OsString::from("~").resolve(), Path::new("/home/test"));
    }

    #[test]
    fn test_resolve_tilde_slash() {
        assert_eq!("~/".resolve(), Path::new("/home/test"));
    }

    #[test]
    fn test_resolve_tilde_path() {
        assert_eq!(
            "~/.config/alacritty/alacritty.yml".resolve(),
            Path::new("/home/test/.config/alacritty/alacritty.yml")
        );
    }

    #[test]
    fn test_resolve_tilde_multislash() {
        assert_eq!("~/////".resolve(), Path::new("/home/test"));
    }

    #[test]
    fn test_resolve_tilde_multislash_path() {
        assert_eq!("~/////.config".resolve(), Path::new("/home/test/.config"));
    }

    #[test]
    fn test_resolve_tilde_with_relative_segments() {
        assert_eq!(
            "~/.config/../.vim/".resolve(),
            Path::new("/home/test/.config/../.vim/")
        )
    }

    #[test]
    fn test_resolve_path() {
        assert_eq!(
            "./config.yml".resolve_in("/home/user/.app"),
            Path::new("/home/user/.app/config.yml")
        );
    }

    #[test]
    fn test_resolve_path_base_trailing_slash() {
        assert_eq!(
            "./config.yml".resolve_in("/home/user/.app/"),
            Path::new("/home/user/.app/config.yml")
        );
    }

    #[test]
    fn test_resolve_path_with_tilde() {
        assert_eq!(
            "./config.yml".resolve_in("~/.app"),
            Path::new("/home/test/.app/config.yml")
        );
    }

    #[test]
    fn test_resolve_absolute_path() {
        assert_eq!(
            "/etc/nixos/configuration.nix".resolve_in("/home/usr/.app"),
            Path::new("/etc/nixos/configuration.nix")
        );
    }

    #[test]
    fn test_resolve_absolute_path2() {
        assert_eq!(
            "~/.config/alacritty/alacritty.yml".resolve_in("/tmp"),
            Path::new("/home/test/.config/alacritty/alacritty.yml")
        );
    }

    #[test]
    fn test_resolve_relative_path() {
        assert_eq!(
            "../.app2/config.yml".resolve_in("/home/user/.app"),
            Path::new("/home/user/.app/../.app2/config.yml")
        );
    }

    #[test]
    fn test_resolve_current_dir() {
        assert_eq!(".".resolve_in("/home/user"), Path::new("/home/user"));
    }

    #[test]
    fn test_resolve_cwd() {
        std::env::set_current_dir("/tmp").unwrap();
        assert_eq!("garbage.txt".resolve(), Path::new("/tmp/garbage.txt"));
    }

    #[test]
    fn test_resolve_base_file() {
        let base_path = "/tmp/path-resolve-test.txt";
        std::fs::write(base_path, "Hello!").unwrap();
        assert_eq!(
            "./other-tmp-file.txt".resolve_in(base_path),
            Path::new("/tmp/other-tmp-file.txt")
        );
    }
}
