# resolve-path

A crate for resolving relative (`./`) and tilde paths (`~/`) in Rust.

Note that this does not perform _path canonicalization_, i.e. it will
not eliminate segments like `..` or `./././` in a path. This crate
is intended simply to anchor relative paths such that they have an
absolute path from the root.

## Motivation

Rust has `Path` and `PathBuf` in the standard library for working with
file paths, but unfortunately there is no easy and ergonomic way to
resolve relative paths in the following ways:

- with respect to the process current-working-directory (CWD)
- with respect to the active user's home directory (`~/`)
- with respect to a user-provided absolute path

## API

This crate provides an extension trait [`PathResolveExt`] with extension
methods for path-like types. The following methods are provided:

### `resolve` and `try_resolve`

These methods will resolve relative paths (`./...`) with respect to the
process current-working-directory, and will also resolve tilde-paths (`~/...`)
to the active user's home directory.

Assuming a home directory of `/home/user` and a CWD of `/home/user/Documents`,
the `resolve` methods will evaluate in the following ways:

```rust
use std::path::Path;
use resolve_path::PathResolveExt;

// Direct variant (may panic)
assert_eq!("~/.vimrc".resolve(), Path::new("/home/user/.vimrc"));
assert_eq!("./notes.txt".resolve(), Path::new("/home/user/Documents/notes.txt"));

// Try variant (returns Result)
assert_eq!("~/.vimrc".try_resolve().unwrap(), Path::new("/home/user/.vimrc"));
assert_eq!("./notes.txt".try_resolve().unwrap(), Path::new("/home/user/Documents/notes.txt"));
```

### `resolve_in` and `try_resolve_in`

These methods will resolve tilde-paths (`~/...`) in the normal way, but will
resolve relative paths (`./...`) with respect to a provided base directory.
This can be very useful, for example when evaluating paths given in a config
file with respect to the location of the config file, rather than with respect
to the process CWD.

Assuming the same home directory of `/home/user` and CWD of `/home/user/Documents`,
the `resolve_in` methods will evaluate in the following ways:

```rust
use std::path::Path;
use resolve_path::PathResolveExt;

// Direct variant (may panic)
assert_eq!("~/.vimrc".resolve_in("~/.config/alacritty/"), Path::new("/home/user/.vimrc"));
assert_eq!("./alacritty.yml".resolve_in("~/.config/alacritty/"), Path::new("/home/user/.config/alacritty/alacritty.yml"));

// Try variant (returns Result)
assert_eq!("~/.vimrc".try_resolve_in("~/.config/alacritty/").unwrap(), Path::new("/home/user/.vimrc"));
assert_eq!("./alacritty.yml".try_resolve_in("~/.config/alacritty/").unwrap(), Path::new("/home/user/.config/alacritty/alacritty.yml"));
```

### Why use `Cow<Path>`?

If any of the [`PathResolveExt`] methods are called on a path that does not
actually need to be resolved (i.e. a path that is already absolute), then
the resolver methods will simply return `Cow::Borrowed(&Path)` with the original
path ref within. If resolution _does_ occur, then the path will one way or another
be edited (e.g. by adding an absolute path prefix), and will be returned as
a `Cow::Owned(PathBuf)`. This way we can avoid allocation where it is unnecessary.
