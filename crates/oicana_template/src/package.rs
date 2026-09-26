use chrono::{Datelike, Timelike, Utc};
use ignore::gitignore::{Gitignore, GitignoreBuilder};
use log::trace;
use serde::Deserialize;
use std::fs::File;
use std::io;
use std::io::{Read, Seek, Write};
use std::num::TryFromIntError;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};
use zip::result::{DateTimeRangeError, ZipError};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::manifest::TemplateManifest;

/// Package a directory as an Oicana template.
pub fn package<T>(
    src_dir: &Path,
    writer: T,
    manifest: &TemplateManifest,
    exclude: Option<&Path>,
) -> Result<(), PackageError>
where
    T: Write + Seek,
{
    let exclude_matcher = manifest.build_exclude_matcher();
    package_with_exclude(src_dir, writer, &exclude_matcher, exclude)
}

/// Package a directory as an Oicana template with a pre-built exclude matcher.
pub fn package_with_exclude<T>(
    src_dir: &Path,
    writer: T,
    exclude_matcher: &Gitignore,
    exclude: Option<&Path>,
) -> Result<(), PackageError>
where
    T: Write + Seek,
{
    package_with_dependencies(src_dir, writer, exclude_matcher, exclude, &[])
}

/// Package a directory as an Oicana template, including extra dependency directories.
///
/// Each entry in `dependencies` is a `(source_dir, zip_prefix)` pair. The contents of
/// `source_dir` will be added to the zip under the given `zip_prefix` path
/// (e.g. `.dependencies/preview/pkg/0.1.0`).
pub fn package_with_dependencies<T>(
    src_dir: &Path,
    writer: T,
    exclude_matcher: &Gitignore,
    exclude: Option<&Path>,
    dependencies: &[(PathBuf, PathBuf)],
) -> Result<(), PackageError>
where
    T: Write + Seek,
{
    if !Path::new(src_dir).is_dir() {
        return Err(PackageError::SourceIsNotADirectory);
    }

    let method = CompressionMethod::ZSTD;
    let mut zip = ZipWriter::new(writer);
    let options = SimpleFileOptions::default()
        .compression_method(method)
        .unix_permissions(0o755);

    // Add template files
    let walk_dir = WalkDir::new(src_dir).follow_links(true);
    let mut it = walk_dir.into_iter().filter_entry(|entry| {
        let relative = entry.path().strip_prefix(src_dir).unwrap();
        if let Some(excluded) = exclude {
            if relative == excluded {
                return false;
            }
        }
        !is_excluded(exclude_matcher, relative, entry)
    });

    add_dir_to_zip(&mut zip, &mut it, src_dir, Path::new(""), options)?;

    // Add dependency directories
    for (source_dir, zip_prefix) in dependencies {
        let dep_matcher = dependency_exclude_matcher(source_dir);
        let dep_walk = WalkDir::new(source_dir).follow_links(true);
        let mut dep_it = dep_walk.into_iter().filter_entry(|entry| {
            let relative = entry.path().strip_prefix(source_dir).unwrap();
            !is_excluded(&dep_matcher, relative, entry)
        });
        add_dir_to_zip(&mut zip, &mut dep_it, source_dir, zip_prefix, options)?;
    }

    zip.finish()?;
    Ok(())
}

fn is_excluded(matcher: &Gitignore, relative: &Path, entry: &DirEntry) -> bool {
    matcher
        .matched_path_or_any_parents(relative, entry.file_type().is_dir())
        .is_ignore()
}

#[derive(Deserialize, Default)]
struct DependencyManifest {
    #[serde(default)]
    package: DependencyPackage,
}

#[derive(Deserialize, Default)]
struct DependencyPackage {
    #[serde(default)]
    exclude: Vec<String>,
}

/// Build a matcher from the `exclude` field of a dependency's `typst.toml`.
fn dependency_exclude_matcher(package_dir: &Path) -> Gitignore {
    let manifest = std::fs::read_to_string(package_dir.join("typst.toml"))
        .ok()
        .and_then(|content| toml::from_str::<DependencyManifest>(&content).ok())
        .unwrap_or_default();
    let mut builder = GitignoreBuilder::new("");
    for pattern in &manifest.package.exclude {
        if let Err(error) = builder.add_line(None, pattern) {
            log::warn!("Ignoring invalid exclude pattern '{pattern}' of a dependency: {error}");
        }
    }
    builder.build().unwrap_or_else(|_| Gitignore::empty())
}

fn add_dir_to_zip<T: Write + Seek>(
    zip: &mut ZipWriter<T>,
    it: &mut dyn Iterator<Item = walkdir::Result<DirEntry>>,
    strip_prefix: &Path,
    zip_prefix: &Path,
    options: SimpleFileOptions,
) -> Result<(), PackageError> {
    let mut buffer = Vec::with_capacity(4096);
    let mut pending_dirs: Vec<PathBuf> = Vec::new();
    for entry in it {
        let entry = entry?;
        let path = entry.path();
        let relative = path.strip_prefix(strip_prefix).unwrap();
        let name = zip_prefix.join(relative);

        if path.is_file() {
            add_parent_dirs_to_zip(zip, &mut pending_dirs, &name, options)?;

            trace!("adding file {:?}", name);
            let mut f = File::open(path)?;
            zip.start_file_from_path(
                &name,
                options.last_modified_time(zip_date_from_system_time(f.metadata()?.modified()?)?),
            )?;

            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
            buffer.clear();
        } else if !name.as_os_str().is_empty() {
            pending_dirs.push(name);
        }
    }
    Ok(())
}

/// Write the not-yet-written directory entries that `file` lives in, keeping the
/// order they were discovered in so a directory precedes its content.
fn add_parent_dirs_to_zip<T: Write + Seek>(
    zip: &mut ZipWriter<T>,
    pending: &mut Vec<PathBuf>,
    file: &Path,
    options: SimpleFileOptions,
) -> Result<(), PackageError> {
    let mut index = 0;
    while index < pending.len() {
        if file.starts_with(&pending[index]) {
            let dir = pending.remove(index);
            trace!("adding dir {:?}", dir);
            zip.add_directory_from_path(&dir, options)?;
        } else {
            index += 1;
        }
    }
    Ok(())
}

fn zip_date_from_system_time(time: SystemTime) -> Result<DateTime, PackageError> {
    let date_time = chrono::DateTime::<Utc>::from(time);
    Ok(DateTime::from_date_and_time(
        date_time.year().try_into()?,
        date_time.month().try_into()?,
        date_time.day().try_into()?,
        date_time.hour().try_into()?,
        date_time.minute().try_into()?,
        date_time.second().try_into()?,
    )?)
}

/// Error while packaging a template.
#[derive(Debug, Error)]
pub enum PackageError {
    /// The given source is not a directory.
    #[error("The source is not a directory")]
    SourceIsNotADirectory,
    /// A file path in the template is not valid UTF-8.
    #[error("File path {0} is not valid UTF-8")]
    InvalidFilePath(PathBuf),
    /// A file or directory in the template could not be read.
    #[error("failed to read template files: {0}")]
    WalkDirectory(#[from] walkdir::Error),
    /// IO Error while packaging the template.
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    /// Error while compressing the template.
    #[error("zip error: {0}")]
    Zip(#[from] ZipError),
    /// Failed to convert a last modified date to a [`DateTime`].
    #[error("failed to convert last modified dates: {0}")]
    IntConversion(#[from] TryFromIntError),
    /// A last modified date is out of range.
    #[error("failed to convert last modified dates: {0}")]
    DateTimeRange(#[from] DateTimeRangeError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::TempDir;

    fn manifest() -> &'static str {
        r#"
[package]
name = "test"
version = "0.1.0"
entrypoint = "main.typ"

[tool.oicana]
manifest_version = 1
"#
    }

    fn create_simple_template() -> TempDir {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("typst.toml"), manifest()).unwrap();
        std::fs::write(dir.path().join("main.typ"), "Hello").unwrap();
        dir
    }

    fn create_template_with_default_excluded_content() -> TempDir {
        let dir = create_simple_template();
        for sub_dir in ["tests", "output", ".git", "assets"] {
            std::fs::create_dir(dir.path().join(sub_dir)).unwrap();
        }
        std::fs::write(dir.path().join("tests").join("test.toml"), "").unwrap();
        std::fs::write(dir.path().join("output").join("main.pdf"), "").unwrap();
        std::fs::write(dir.path().join(".git").join("config"), "").unwrap();
        std::fs::write(dir.path().join(".DS_Store"), "").unwrap();
        std::fs::write(dir.path().join("demo-0.1.0.zip"), "").unwrap();
        std::fs::write(dir.path().join("assets").join(".DS_Store"), "").unwrap();
        std::fs::write(dir.path().join("assets").join("logo.svg"), "<svg/>").unwrap();
        dir
    }

    #[test]
    fn packages_simple_template_and_can_unpack() {
        let dir = create_simple_template();
        let manifest = TemplateManifest::from_toml(
            &std::fs::read_to_string(dir.path().join("typst.toml")).unwrap(),
        )
        .unwrap();

        let mut buffer = Cursor::new(Vec::new());
        package(dir.path(), &mut buffer, &manifest, None).unwrap();

        buffer.set_position(0);
        let mut archive = zip::ZipArchive::new(buffer).unwrap();

        assert!(archive.by_name("main.typ").is_ok());
        assert!(archive.by_name("typst.toml").is_ok());

        let mut main_file = archive.by_name("main.typ").unwrap();
        let mut content = String::new();
        main_file.read_to_string(&mut content).unwrap();
        assert_eq!(content, "Hello");
    }

    #[test]
    fn packages_template_with_subdirectories() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("typst.toml"), manifest()).unwrap();
        std::fs::write(dir.path().join("main.typ"), "Hello").unwrap();
        std::fs::create_dir(dir.path().join("assets")).unwrap();
        std::fs::write(dir.path().join("assets").join("data.json"), "{}").unwrap();

        let manifest = TemplateManifest::from_toml(
            &std::fs::read_to_string(dir.path().join("typst.toml")).unwrap(),
        )
        .unwrap();

        let mut buffer = Cursor::new(Vec::new());
        package(dir.path(), &mut buffer, &manifest, None).unwrap();

        buffer.set_position(0);
        let mut archive = zip::ZipArchive::new(buffer).unwrap();

        let mut file = archive
            .by_name("assets/data.json")
            .expect("File should be part of the archive");
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        assert_eq!(content, "{}");
    }

    #[test]
    fn applies_the_exclude_list_of_dependencies() {
        let template = TempDir::new().unwrap();
        std::fs::write(template.path().join("typst.toml"), manifest()).unwrap();
        std::fs::write(template.path().join("main.typ"), "Hello").unwrap();

        let dependency = TempDir::new().unwrap();
        std::fs::write(
            dependency.path().join("typst.toml"),
            "[package]\nname = \"dep\"\nversion = \"0.1.0\"\nentrypoint = \"lib.typ\"\nexclude = [\"tests/**\", \".github\"]\n",
        )
        .unwrap();
        std::fs::write(dependency.path().join("lib.typ"), "").unwrap();
        for dir in ["tests", ".github"] {
            std::fs::create_dir(dependency.path().join(dir)).unwrap();
            std::fs::write(dependency.path().join(dir).join("file"), "").unwrap();
        }

        let manifest = TemplateManifest::from_toml(manifest()).unwrap();
        let mut buffer = Cursor::new(Vec::new());
        package_with_dependencies(
            template.path(),
            &mut buffer,
            &manifest.build_exclude_matcher(),
            None,
            &[(
                dependency.path().to_path_buf(),
                PathBuf::from(".dependencies/preview/dep/0.1.0"),
            )],
        )
        .unwrap();

        buffer.set_position(0);
        let archive = zip::ZipArchive::new(buffer).unwrap();
        let names: Vec<&str> = archive.file_names().collect();
        assert!(names.contains(&".dependencies/preview/dep/0.1.0/lib.typ"));
        assert!(names.contains(&".dependencies/preview/dep/0.1.0/typst.toml"));
        assert!(!names
            .iter()
            .any(|name| name.contains("tests") || name.contains(".github")));
    }

    #[test]
    fn excludes_defaults_from_the_archive() {
        let dir = create_template_with_default_excluded_content();
        let manifest = TemplateManifest::from_toml(
            &std::fs::read_to_string(dir.path().join("typst.toml")).unwrap(),
        )
        .unwrap();

        let mut buffer = Cursor::new(Vec::new());
        package(dir.path(), &mut buffer, &manifest, None).unwrap();

        buffer.set_position(0);
        let archive = zip::ZipArchive::new(buffer).unwrap();
        let names: Vec<&str> = archive.file_names().collect();

        assert!(names.contains(&"main.typ"), "{names:?}");
        assert!(names.contains(&"assets/logo.svg"), "{names:?}");
        for excluded in ["tests", "output", ".git", ".DS_Store", ".zip"] {
            assert!(
                !names.iter().any(|name| name.contains(excluded)),
                "expected no {excluded} entry in {names:?}"
            );
        }
    }

    #[test]
    fn default_excludes_can_be_re_included() {
        let dir = create_template_with_default_excluded_content();
        let mut manifest = TemplateManifest::from_toml(
            &std::fs::read_to_string(dir.path().join("typst.toml")).unwrap(),
        )
        .unwrap();
        manifest.package.exclude = ["!/tests/", "!/output/", "!.git", "!.DS_Store", "!*.zip"]
            .map(Into::into)
            .to_vec();

        let mut buffer = Cursor::new(Vec::new());
        package(dir.path(), &mut buffer, &manifest, None).unwrap();

        buffer.set_position(0);
        let archive = zip::ZipArchive::new(buffer).unwrap();
        let names: Vec<&str> = archive.file_names().collect();

        for re_included in [
            "tests/test.toml",
            "output/main.pdf",
            ".git/config",
            ".DS_Store",
            "demo-0.1.0.zip",
            "assets/.DS_Store",
        ] {
            assert!(
                names.contains(&re_included),
                "expected {re_included} in {names:?}"
            );
        }
    }

    #[test]
    fn directories_without_packed_content_are_left_out() {
        let dir = TempDir::new().unwrap();
        std::fs::write(
            dir.path().join("typst.toml"),
            r#"
[package]
name = "test"
version = "0.1.0"
entrypoint = "main.typ"
exclude = ["docs/*.pdf"]

[tool.oicana]
manifest_version = 1
"#,
        )
        .unwrap();
        std::fs::write(dir.path().join("main.typ"), "Hello").unwrap();
        // Every file in here is excluded, so the directory holds nothing.
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs").join("manual.pdf"), "%PDF").unwrap();
        // A directory that is empty on disk.
        std::fs::create_dir(dir.path().join("scratch")).unwrap();
        // A directory that keeps content is still packed, entry first.
        std::fs::create_dir_all(dir.path().join("assets/nested")).unwrap();
        std::fs::write(dir.path().join("assets/nested/data.json"), "{}").unwrap();

        let manifest = TemplateManifest::from_toml(
            &std::fs::read_to_string(dir.path().join("typst.toml")).unwrap(),
        )
        .unwrap();

        let mut buffer = Cursor::new(Vec::new());
        package(dir.path(), &mut buffer, &manifest, None).unwrap();

        buffer.set_position(0);
        let archive = zip::ZipArchive::new(buffer).unwrap();
        let names: Vec<String> = archive.file_names().map(|name| name.to_owned()).collect();

        assert!(
            !names.iter().any(|name| name.starts_with("docs")),
            "{names:?}"
        );
        assert!(
            !names.iter().any(|name| name.starts_with("scratch")),
            "{names:?}"
        );

        let dir_position = names.iter().position(|name| name == "assets/nested/");
        let file_position = names
            .iter()
            .position(|name| name == "assets/nested/data.json");
        assert!(dir_position < file_position, "{names:?}");
        assert!(names.contains(&"assets/".to_owned()), "{names:?}");
    }

    #[test]
    fn fails_when_source_is_not_directory() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("not_a_dir.txt");
        std::fs::write(&file_path, "content").unwrap();

        let manifest = TemplateManifest::from_toml(manifest()).unwrap();
        let mut buffer = Cursor::new(Vec::new());
        let result = package(&file_path, &mut buffer, &manifest, None);

        assert!(matches!(result, Err(PackageError::SourceIsNotADirectory)));
    }

    #[test]
    fn fails_when_source_does_not_exist() {
        let manifest = TemplateManifest::from_toml(manifest()).unwrap();
        let mut buffer = Cursor::new(Vec::new());
        let result = package(Path::new("/nonexistent/path"), &mut buffer, &manifest, None);

        assert!(result.is_err());
    }

    #[test]
    fn packed_template_uses_forward_slashes_in_paths() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("typst.toml"), manifest()).unwrap();
        std::fs::write(dir.path().join("main.typ"), "Main content").unwrap();
        std::fs::create_dir(dir.path().join("lib")).unwrap();
        std::fs::write(dir.path().join("lib").join("utils.typ"), "Utils content").unwrap();
        std::fs::create_dir(dir.path().join("assets")).unwrap();
        std::fs::write(dir.path().join("assets").join("data.json"), "{}").unwrap();
        std::fs::create_dir(dir.path().join("assets").join("images")).unwrap();
        std::fs::write(
            dir.path().join("assets").join("images").join("logo.txt"),
            "Logo placeholder",
        )
        .unwrap();

        let manifest = TemplateManifest::from_toml(
            &std::fs::read_to_string(dir.path().join("typst.toml")).unwrap(),
        )
        .unwrap();

        let mut buffer = Cursor::new(Vec::new());
        package(dir.path(), &mut buffer, &manifest, None).unwrap();

        buffer.set_position(0);
        let mut archive = zip::ZipArchive::new(buffer).unwrap();

        let mut paths_with_backslashes = Vec::new();
        let mut all_paths = Vec::new();

        for i in 0..archive.len() {
            let entry = archive.by_index(i).unwrap();
            let name = entry.name().to_string();
            all_paths.push(name.clone());

            if name.contains('\\') {
                paths_with_backslashes.push(name);
            }
        }

        assert!(
            paths_with_backslashes.is_empty(),
            "ZIP paths must use forward slashes '/' not backslashes '\\'. Found paths with backslashes: {:?}",
            paths_with_backslashes
        );

        assert!(
            all_paths.iter().any(|p| p == "main.typ"),
            "Expected 'main.typ' in zip"
        );
        assert!(
            all_paths.iter().any(|p| p == "typst.toml"),
            "Expected 'typst.toml' in zip"
        );
        assert!(
            all_paths.iter().any(|p| p == "lib/utils.typ"),
            "Expected 'lib/utils.typ' with forward slash in zip"
        );
        assert!(
            all_paths.iter().any(|p| p == "assets/data.json"),
            "Expected 'assets/data.json' with forward slash in zip"
        );
        assert!(
            all_paths.iter().any(|p| p == "assets/images/logo.txt"),
            "Expected 'assets/images/logo.txt' with forward slashes in zip"
        );
    }

    fn symlink_dir(src: &Path, dst: &Path) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(src, dst).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(src, dst).unwrap();
    }

    fn symlink_file(src: &Path, dst: &Path) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(src, dst).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(src, dst).unwrap();
    }

    #[test]
    fn packages_symlinked_directory_contents() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("typst.toml"), manifest()).unwrap();
        std::fs::write(dir.path().join("main.typ"), "Hello").unwrap();

        // Create an external directory with files and symlink it into the template
        let external = TempDir::new().unwrap();
        std::fs::create_dir(external.path().join("lib")).unwrap();
        std::fs::write(external.path().join("lib").join("utils.typ"), "// utils").unwrap();
        symlink_dir(&external.path().join("lib"), &dir.path().join("lib"));

        let manifest = TemplateManifest::from_toml(
            &std::fs::read_to_string(dir.path().join("typst.toml")).unwrap(),
        )
        .unwrap();

        let mut buffer = Cursor::new(Vec::new());
        package(dir.path(), &mut buffer, &manifest, None).unwrap();

        buffer.set_position(0);
        let mut archive = zip::ZipArchive::new(buffer).unwrap();

        let mut utils = archive
            .by_name("lib/utils.typ")
            .expect("File inside symlinked directory should be packed");
        let mut content = String::new();
        utils.read_to_string(&mut content).unwrap();
        assert_eq!(content, "// utils");
    }

    #[test]
    fn packages_symlinked_files_with_their_content() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("typst.toml"), manifest()).unwrap();
        std::fs::write(dir.path().join("main.typ"), "Hello").unwrap();

        // Create an external file and symlink to it from inside the template
        let external = TempDir::new().unwrap();
        std::fs::write(external.path().join("data.json"), r#"{"key": "value"}"#).unwrap();
        symlink_file(
            &external.path().join("data.json"),
            &dir.path().join("linked.json"),
        );

        let manifest = TemplateManifest::from_toml(
            &std::fs::read_to_string(dir.path().join("typst.toml")).unwrap(),
        )
        .unwrap();

        let mut buffer = Cursor::new(Vec::new());
        package(dir.path(), &mut buffer, &manifest, None).unwrap();

        buffer.set_position(0);
        let mut archive = zip::ZipArchive::new(buffer).unwrap();

        // The symlinked file should be included with its actual content
        let mut linked = archive
            .by_name("linked.json")
            .expect("Symlinked file should be packed into the zip");
        let mut content = String::new();
        linked.read_to_string(&mut content).unwrap();
        assert_eq!(content, r#"{"key": "value"}"#);
    }

    #[test]
    fn fails_on_a_symlink_that_cannot_be_read() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("typst.toml"), manifest()).unwrap();
        std::fs::write(dir.path().join("main.typ"), "Hello").unwrap();
        symlink_file(
            Path::new("does-not-exist.json"),
            &dir.path().join("linked.json"),
        );

        let manifest = TemplateManifest::from_toml(
            &std::fs::read_to_string(dir.path().join("typst.toml")).unwrap(),
        )
        .unwrap();

        let mut buffer = Cursor::new(Vec::new());
        let error = package(dir.path(), &mut buffer, &manifest, None)
            .expect_err("A symlink that cannot be followed should fail the pack");
        assert!(matches!(error, PackageError::WalkDirectory(_)));
    }
}
