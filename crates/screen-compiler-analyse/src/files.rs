// SPDX-License-Identifier: MIT
//! The files a screen imports its components from.
//!
//! A screen may import components, design tokens and lists from `.tsx`
//! files in its own folder or below it, by a relative path:
//! `import { Header } from "./parts/header";`. Each is read once, through a
//! [`Load`]er, whatever imports it; a screen and the files it reads number at
//! most [`MAX_FILES`], each within the parser's size cap. Every name is
//! resolved in the file it is written in, as in JavaScript, and every
//! location carries its file, so an error inside an imported component is
//! reported in that file.
//!
//! [`Folder`] reads from a folder on disk and nothing outside it: a path
//! that leaves it, through `..` or a symbolic link, is refused.

use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// The most files one screen reads: its own and every file it imports,
/// directly or through another.
pub const MAX_FILES: usize = 32;

/// Why a file could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// No such file.
    Missing,
    /// The path leaves the screen's folder (through a symbolic link) or
    /// names something other than a file.
    Outside,
    /// Larger than the parser takes.
    TooLarge,
    /// Not UTF-8.
    NotUtf8,
    /// No files are read: the screen was given as text alone.
    NotGiven,
    /// Any other failure, as the system describes it.
    Unreadable(String),
}

impl LoadError {
    /// Why, as the end of a sentence.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Missing => "there is no such file".to_owned(),
            Self::Outside => {
                "it is not a file inside the screen's folder (a symbolic link may lead out)"
                    .to_owned()
            }
            Self::TooLarge => format!(
                "it is larger than {} bytes",
                screen_compiler_tsx::MAX_SOURCE_BYTES
            ),
            Self::NotUtf8 => "it is not valid UTF-8".to_owned(),
            Self::NotGiven => "only a screen read from a folder can import files".to_owned(),
            Self::Unreadable(why) => why.clone(),
        }
    }
}

/// Reads the files a screen imports.
pub trait Load {
    /// The text of `path`: a path from the screen's folder, its parts
    /// separated by `/`, none of them empty, `.` or `..`.
    ///
    /// # Errors
    ///
    /// [`LoadError`] when it cannot be read.
    fn load(&mut self, path: &str) -> Result<String, LoadError>;
}

/// Reads no files: for a screen given as text alone.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoFiles;

impl Load for NoFiles {
    fn load(&mut self, _path: &str) -> Result<String, LoadError> {
        Err(LoadError::NotGiven)
    }
}

/// Reads files from one folder on disk and below it, and nothing else.
#[derive(Clone, Debug)]
pub struct Folder {
    /// The folder, with every symbolic link resolved.
    root: PathBuf,
}

impl Folder {
    /// A loader for the files in `dir` and below.
    ///
    /// # Errors
    ///
    /// When `dir` cannot be resolved.
    pub fn new(dir: &Path) -> io::Result<Self> {
        let dir = if dir.as_os_str().is_empty() {
            Path::new(".")
        } else {
            dir
        };
        Ok(Self {
            root: fs::canonicalize(dir)?,
        })
    }

    /// The loader for the folder `file` is in.
    ///
    /// # Errors
    ///
    /// When that folder cannot be resolved.
    pub fn of(file: &Path) -> io::Result<Self> {
        Self::new(file.parent().unwrap_or(Path::new(".")))
    }
}

impl Load for Folder {
    fn load(&mut self, path: &str) -> Result<String, LoadError> {
        let mut joined = self.root.clone();
        for part in path.split('/') {
            if part.is_empty() || part == "." || part == ".." {
                return Err(LoadError::Outside);
            }
            joined.push(part);
        }
        let real = fs::canonicalize(&joined).map_err(|error| match error.kind() {
            io::ErrorKind::NotFound => LoadError::Missing,
            _ => LoadError::Unreadable(error.to_string()),
        })?;
        if !real.starts_with(&self.root) {
            return Err(LoadError::Outside);
        }
        let file = File::open(&real).map_err(|error| LoadError::Unreadable(error.to_string()))?;
        let is_file = file
            .metadata()
            .map_err(|error| LoadError::Unreadable(error.to_string()))?
            .is_file();
        if !is_file {
            return Err(LoadError::Outside);
        }
        let limit = u64::try_from(screen_compiler_tsx::MAX_SOURCE_BYTES)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let mut bytes = Vec::new();
        file.take(limit)
            .read_to_end(&mut bytes)
            .map_err(|error| LoadError::Unreadable(error.to_string()))?;
        if bytes.len() > screen_compiler_tsx::MAX_SOURCE_BYTES {
            return Err(LoadError::TooLarge);
        }
        String::from_utf8(bytes).map_err(|_| LoadError::NotUtf8)
    }
}

/// Whether a module specifier names a file of the project's own rather
/// than a package: a path, relative or absolute.
pub(crate) fn is_path(specifier: &str) -> bool {
    specifier.starts_with('.') || specifier.starts_with('/')
}

/// Extensions of files that are not TSX, which are never read.
const OTHER_EXTENSIONS: [&str; 9] = [
    ".ts", ".js", ".jsx", ".mjs", ".cjs", ".mts", ".cts", ".json", ".d.ts",
];

/// The path from the screen's folder that `specifier`, imported by the file
/// at `from`, names; or why it names none Screen Compiler reads, with help.
pub(crate) fn resolve(from: &str, specifier: &str) -> Result<String, (String, String)> {
    let Some(rest) = specifier.strip_prefix("./") else {
        return Err((
            format!("\"{specifier}\" is outside the screen's folder"),
            "a screen imports only from its own folder and below, by a path starting `./`: put the component beside the screen".to_owned(),
        ));
    };
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in rest.split('/') {
        let plain = !part.is_empty()
            && part != "."
            && part != ".."
            && !part.contains(['\\', ':'])
            && !part.chars().any(|c| c.is_control());
        if !plain {
            return Err((
                format!("\"{specifier}\" is not a plain path below the screen's folder"),
                "write the path from the importing file: ./header or ./parts/header".to_owned(),
            ));
        }
        parts.push(part);
    }
    let mut path = parts.join("/");
    if !path.ends_with(".tsx") {
        if let Some(extension) = OTHER_EXTENSIONS.iter().find(|ext| path.ends_with(*ext)) {
            return Err((
                format!("\"{specifier}\" is a {extension} file: only .tsx files are read"),
                "write the components in a .tsx file and import it without its extension: ./header"
                    .to_owned(),
            ));
        }
        path.push_str(".tsx");
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        reason = "test code: a failed check panics"
    )]

    use super::*;

    #[test]
    fn a_path_is_resolved_from_the_importing_file() {
        assert_eq!(resolve("screen.tsx", "./header").unwrap(), "header.tsx");
        assert_eq!(resolve("screen.tsx", "./header.tsx").unwrap(), "header.tsx");
        assert_eq!(
            resolve("screen.tsx", "./parts/header").unwrap(),
            "parts/header.tsx"
        );
        assert_eq!(
            resolve("parts/header.tsx", "./badge").unwrap(),
            "parts/badge.tsx"
        );
        assert_eq!(
            resolve("screen.tsx", "./card.view").unwrap(),
            "card.view.tsx"
        );
    }

    #[test]
    fn paths_that_leave_the_folder_or_name_no_tsx_file_are_refused() {
        for specifier in [
            "../header",
            "/abs/header",
            ".header",
            "./parts/../header",
            "./a//b",
            "./a/./b",
            "./",
            "./a\\b",
            "./c:x",
            "./a\u{7}",
            "./header.js",
            "./header.ts",
            "./data.json",
        ] {
            assert!(
                resolve("screen.tsx", specifier).is_err(),
                "{specifier} is refused"
            );
        }
    }

    #[test]
    fn a_folder_reads_only_files_inside_it() {
        let base =
            std::env::temp_dir().join(format!("screen-compiler-files-{}", std::process::id()));
        let inner = base.join("screens");
        fs::create_dir_all(inner.join("parts")).unwrap();
        fs::write(inner.join("parts/header.tsx"), "export const A = 1;\n").unwrap();
        fs::write(base.join("secret.tsx"), "outside\n").unwrap();
        fs::write(
            inner.join("big.tsx"),
            "x".repeat(screen_compiler_tsx::MAX_SOURCE_BYTES + 1),
        )
        .unwrap();
        fs::write(inner.join("latin1.tsx"), [0xe9_u8]).unwrap();
        let mut folder = Folder::new(&inner).unwrap();
        assert_eq!(
            folder.load("parts/header.tsx").unwrap(),
            "export const A = 1;\n"
        );
        assert_eq!(folder.load("missing.tsx"), Err(LoadError::Missing));
        assert_eq!(folder.load("../secret.tsx"), Err(LoadError::Outside));
        assert_eq!(folder.load("parts"), Err(LoadError::Outside));
        assert_eq!(folder.load("big.tsx"), Err(LoadError::TooLarge));
        assert_eq!(folder.load("latin1.tsx"), Err(LoadError::NotUtf8));
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(base.join("secret.tsx"), inner.join("link.tsx")).unwrap();
            assert_eq!(folder.load("link.tsx"), Err(LoadError::Outside));
        }
        fs::remove_dir_all(&base).unwrap();
    }
}
