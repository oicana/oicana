use crate::TemplateFiles;
use log::warn;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek};
use std::str::FromStr;
use std::sync::{Mutex, PoisonError};
use std::{str, vec};
use thiserror::Error;
use typst::diag::{eco_format, FileError, FileResult, PackageError};
use typst::foundations::Bytes;
use typst::syntax::package::{PackageSpec, PackageVersion};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use zip::read::ZipFile;
use zip::ZipArchive;

/// An error that occurred while reading a packed template.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum PackedTemplateError {
    /// The zip archive could not be read.
    #[error("Failed to read archive: {0}")]
    InvalidArchive(#[from] zip::result::ZipError),
    /// The template archive contains more entries than allowed by [`ZipLimits`].
    #[error("Template archive has {count} entries, exceeding the limit of {limit}")]
    TooManyEntries {
        /// The number of entries in the archive.
        count: usize,
        /// The configured entry limit.
        limit: usize,
    },
    /// The decompressed template archive content exceeds the size allowed by [`ZipLimits`].
    #[error("Decompressed template archive content exceeds the limit of {limit} bytes")]
    TooLarge {
        /// The configured limit for the total decompressed size in bytes.
        limit: u64,
    },
    /// An archive entry uses a compression method other than stored, deflate or zstd.
    #[error(
        "Template archive entry '{path}' uses the unsupported compression method {method}. \
         Supported methods are stored, deflate and zstd; `oicana pack` creates compatible archives."
    )]
    UnsupportedCompression {
        /// The path of the entry in the archive.
        path: String,
        /// The zip compression method id of the entry.
        method: u16,
    },
}

/// A zip limit was given as a negative number, which cannot express a bound.
#[derive(Error, Debug, PartialEq, Eq)]
#[error("{bound} must not be negative, got {value}")]
pub struct NegativeZipLimit {
    /// The bound that was negative, named as the integrations expose it.
    pub bound: &'static str,
    /// The value that was passed.
    pub value: i64,
}

/// Limits applied while reading a packed template archive.
#[derive(Debug, Clone, Copy)]
pub struct ZipLimits {
    /// Maximum number of entries in the archive.
    pub max_entries: usize,
    /// Maximum total decompressed size of all entries in bytes.
    pub max_total_decompressed_bytes: u64,
}

impl Default for ZipLimits {
    fn default() -> Self {
        ZipLimits {
            max_entries: 10_000,
            max_total_decompressed_bytes: 512 * 1024 * 1024,
        }
    }
}

impl ZipLimits {
    /// Build limits from optional bounds, keeping the default for every bound that is `None`.
    pub fn from_optional(
        max_entries: Option<u64>,
        max_total_decompressed_bytes: Option<u64>,
    ) -> Option<Self> {
        if max_entries.is_none() && max_total_decompressed_bytes.is_none() {
            return None;
        }
        let defaults = ZipLimits::default();
        Some(ZipLimits {
            max_entries: max_entries
                .map(|entries| usize::try_from(entries).unwrap_or(usize::MAX))
                .unwrap_or(defaults.max_entries),
            max_total_decompressed_bytes: max_total_decompressed_bytes
                .unwrap_or(defaults.max_total_decompressed_bytes),
        })
    }

    /// Build limits from signed bounds, rejecting negative values.
    /// For ABIs that can express absence but carry the bounds as signed integers.
    pub fn from_signed(
        max_entries: Option<i64>,
        max_total_decompressed_bytes: Option<i64>,
    ) -> Result<Option<Self>, NegativeZipLimit> {
        fn checked(
            bound: &'static str,
            value: Option<i64>,
        ) -> Result<Option<u64>, NegativeZipLimit> {
            value
                .map(|value| u64::try_from(value).map_err(|_| NegativeZipLimit { bound, value }))
                .transpose()
        }
        Ok(Self::from_optional(
            checked("maxEntries", max_entries)?,
            checked("maxTotalDecompressedBytes", max_total_decompressed_bytes)?,
        ))
    }

    /// Build limits for ABIs that cannot express absence, where a negative bound keeps the
    /// default. See [`ZipLimits::from_optional`].
    pub fn from_sentinels(max_entries: i64, max_total_decompressed_bytes: i64) -> Option<Self> {
        Self::from_optional(
            u64::try_from(max_entries).ok(),
            u64::try_from(max_total_decompressed_bytes).ok(),
        )
    }

    /// Check the sizes declared in an archive's central directory against these limits.
    pub fn check_declared<R: Read + Seek>(&self, reader: R) -> Result<(), PackedTemplateError> {
        let mut archive = ZipArchive::new(reader).map_err(PackedTemplateError::InvalidArchive)?;
        if archive.len() > self.max_entries {
            return Err(PackedTemplateError::TooManyEntries {
                count: archive.len(),
                limit: self.max_entries,
            });
        }
        let mut total_bytes: u64 = 0;
        for index in 0..archive.len() {
            let entry = archive
                .by_index_raw(index)
                .map_err(PackedTemplateError::InvalidArchive)?;
            total_bytes = total_bytes.saturating_add(entry.size());
        }
        if total_bytes > self.max_total_decompressed_bytes {
            return Err(PackedTemplateError::TooLarge {
                limit: self.max_total_decompressed_bytes,
            });
        }
        Ok(())
    }
}

/// A packed template.
///
/// All source and byte entries are always loaded in memory.
pub struct PackedTemplate {
    source: Mutex<HashMap<FileId, Source>>,
    bytes: Mutex<HashMap<FileId, Bytes>>,
    fonts: Vec<FileId>,
    packages: HashSet<PackageSpec>,
}

impl PackedTemplate {
    /// Create a new packed template from a reader of a zip file.
    pub fn new<R: Read + Seek>(reader: R) -> Result<Self, PackedTemplateError> {
        Self::new_with_limits(reader, ZipLimits::default())
    }

    /// Create a new packed template from a reader of a zip file, enforcing the given limits.
    pub fn new_with_limits<R: Read + Seek>(
        reader: R,
        limits: ZipLimits,
    ) -> Result<Self, PackedTemplateError> {
        let mut archive = ZipArchive::new(reader).map_err(PackedTemplateError::InvalidArchive)?;
        if archive.len() > limits.max_entries {
            return Err(PackedTemplateError::TooManyEntries {
                count: archive.len(),
                limit: limits.max_entries,
            });
        }
        let mut remaining_bytes = limits.max_total_decompressed_bytes;

        let mut source = HashMap::new();
        let mut bytes = HashMap::new();
        let mut fonts = vec![];
        let mut packages = HashSet::new();
        let paths: Vec<String> = archive
            .file_names()
            .filter_map(|path| match path {
                Ok(path) => Some(path.into_owned()),
                Err(error) => {
                    warn!("Failed to decode a zip path: {error}");
                    None
                }
            })
            .collect();
        for path in &paths {
            let file_result = archive.by_name(path);
            let content = match file_result {
                Ok(content) => content,
                Err(zip::result::ZipError::CompressionMethodNotSupported(method)) => {
                    return Err(PackedTemplateError::UnsupportedCompression {
                        path: path.clone(),
                        method,
                    });
                }
                Err(error) => {
                    warn!("Failed to read zip path {path}: {error}");
                    continue;
                }
            };
            if !content.is_file() {
                continue;
            }

            if let Some((dir, path)) = path.split_once("/") {
                if dir == ".dependencies" {
                    let Some((namespace, path)) = path.split_once("/") else {
                        warn!("No namespace for dependency path {path}");
                        continue;
                    };
                    let Some((package, path)) = path.split_once("/") else {
                        warn!("No package for dependency path {path}");
                        continue;
                    };
                    let Some((version, path)) = path.split_once("/") else {
                        warn!("No version for dependency path {path}");
                        continue;
                    };
                    let version = match PackageVersion::from_str(version) {
                        Ok(version) => version,
                        Err(error) => {
                            warn!(
                                "Skipping package file {path}, because version cannot be parsed: {error}"
                            );
                            continue;
                        }
                    };
                    let vpath = match VirtualPath::new(path) {
                        Ok(vpath) => vpath,
                        Err(error) => {
                            warn!("Skipping package file with invalid path {path}: {error}");
                            continue;
                        }
                    };
                    let spec = PackageSpec {
                        version,
                        name: package.into(),
                        namespace: namespace.into(),
                    };
                    packages.insert(spec.clone());
                    let id = FileId::new(RootedPath::new(VirtualRoot::Package(spec), vpath));
                    if is_font(path) {
                        fonts.push(id);
                    }
                    read_zip_file_content(
                        &mut source,
                        &mut bytes,
                        content,
                        id,
                        &mut remaining_bytes,
                        &limits,
                    )?;
                    continue;
                }
            };

            let vpath = match VirtualPath::new(path) {
                Ok(vpath) => vpath,
                Err(error) => {
                    warn!("Skipping file with invalid path {path}: {error}");
                    continue;
                }
            };
            let id = FileId::new(RootedPath::new(VirtualRoot::Project, vpath));
            if is_font(path) {
                fonts.push(id);
            }
            read_zip_file_content(
                &mut source,
                &mut bytes,
                content,
                id,
                &mut remaining_bytes,
                &limits,
            )?;
        }

        Ok(PackedTemplate {
            source: Mutex::new(source),
            bytes: Mutex::new(bytes),
            fonts,
            packages,
        })
    }

    fn not_found(&self, id: FileId) -> FileError {
        match id.root() {
            VirtualRoot::Package(spec) if !self.packages.contains(spec) => {
                FileError::Package(PackageError::Other(Some(eco_format!(
                    "{spec} is not part of the template archive. \
                     `oicana pack` only bundles packages imported with a string literal, \
                     like `#import \"{spec}\"`"
                ))))
            }
            _ => FileError::NotFound(id.vpath().get_with_slash().into()),
        }
    }
}

fn read_zip_file_content<R: Read + Seek>(
    source: &mut HashMap<FileId, Source>,
    bytes: &mut HashMap<FileId, Bytes>,
    content: ZipFile<R>,
    id: FileId,
    remaining_bytes: &mut u64,
    limits: &ZipLimits,
) -> Result<(), PackedTemplateError> {
    let mut buffer = Vec::with_capacity(8192);
    let mut limited = content.take(remaining_bytes.saturating_add(1));
    if let Err(error) = limited.read_to_end(&mut buffer) {
        warn!(
            "Failed to read zip file content for {}: {error}",
            id.vpath().get_with_slash()
        );
        return Ok(());
    }
    if buffer.len() as u64 > *remaining_bytes {
        return Err(PackedTemplateError::TooLarge {
            limit: limits.max_total_decompressed_bytes,
        });
    }
    *remaining_bytes -= buffer.len() as u64;
    if let Ok(string_content) = str::from_utf8(&buffer) {
        source.insert(id, Source::new(id, string_content.to_owned()));
    }
    bytes.insert(id, Bytes::new(buffer));
    Ok(())
}

fn is_font(path: &str) -> bool {
    let path = path.to_lowercase();
    path.ends_with(".ttf")
        || path.ends_with(".ttc")
        || path.ends_with(".otf")
        || path.ends_with(".otc")
}

impl TemplateFiles for PackedTemplate {
    fn source(&self, id: FileId) -> FileResult<Source> {
        // The maps are never structurally modified after construction, so a
        // poisoned lock cannot mean inconsistent data.
        let map = self.source.lock().unwrap_or_else(PoisonError::into_inner);
        map.get(&id).cloned().ok_or_else(|| self.not_found(id))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        let map = self.bytes.lock().unwrap_or_else(PoisonError::into_inner);
        map.get(&id).cloned().ok_or_else(|| self.not_found(id))
    }

    fn font_files(&self) -> &Vec<FileId> {
        &self.fonts
    }
}

#[cfg(test)]
mod tests {
    use crate::packed::{NegativeZipLimit, PackedTemplate, PackedTemplateError, ZipLimits};
    use crate::TemplateFiles;
    use std::fs::read;
    use std::io::{Cursor, Write};
    use typst::diag::{EcoString, FileError};
    use typst::syntax::package::{PackageManifest, PackageSpec};
    use typst::syntax::{FileId, RootedPath, VirtualPath, VirtualRoot};
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    #[test]
    fn no_bound_means_defaults() {
        assert!(ZipLimits::from_optional(None, None).is_none());
        assert!(ZipLimits::from_sentinels(-1, -1).is_none());
    }

    #[test]
    fn an_unset_bound_keeps_its_default() {
        let defaults = ZipLimits::default();

        let entries_only = ZipLimits::from_optional(Some(5), None).expect("limits");
        assert_eq!(entries_only.max_entries, 5);
        assert_eq!(
            entries_only.max_total_decompressed_bytes,
            defaults.max_total_decompressed_bytes
        );

        let bytes_only = ZipLimits::from_sentinels(-1, 128).expect("limits");
        assert_eq!(bytes_only.max_entries, defaults.max_entries);
        assert_eq!(bytes_only.max_total_decompressed_bytes, 128);

        let entries_only = ZipLimits::from_sentinels(10, -1).expect("limits");
        assert_eq!(entries_only.max_entries, 10);
        assert_eq!(
            entries_only.max_total_decompressed_bytes,
            defaults.max_total_decompressed_bytes
        );
    }

    #[test]
    fn a_bound_beyond_usize_saturates_instead_of_wrapping() {
        let limits = ZipLimits::from_optional(Some(u64::MAX), Some(u64::MAX)).expect("limits");
        assert_eq!(limits.max_entries, usize::MAX);
        assert_eq!(limits.max_total_decompressed_bytes, u64::MAX);
    }

    #[test]
    fn signed_bounds_reject_negative_values() {
        assert_eq!(
            ZipLimits::from_signed(Some(-1), None).unwrap_err(),
            NegativeZipLimit {
                bound: "maxEntries",
                value: -1,
            }
        );
        assert_eq!(
            ZipLimits::from_signed(None, Some(-8)).unwrap_err().bound,
            "maxTotalDecompressedBytes"
        );
        assert!(ZipLimits::from_signed(None, None).unwrap().is_none());

        let limits = ZipLimits::from_signed(Some(3), None)
            .unwrap()
            .expect("limits");
        assert_eq!(limits.max_entries, 3);
    }

    #[test]
    fn zero_is_a_bound_not_an_absent_value() {
        let limits = ZipLimits::from_optional(Some(0), Some(0)).expect("limits");
        assert_eq!(limits.max_entries, 0);
        assert_eq!(limits.max_total_decompressed_bytes, 0);
    }

    fn project_file(path: &str) -> FileId {
        FileId::new(RootedPath::new(
            VirtualRoot::Project,
            VirtualPath::new(path).unwrap(),
        ))
    }

    fn package_file(spec: PackageSpec, path: &str) -> FileId {
        FileId::new(RootedPath::new(
            VirtualRoot::Package(spec),
            VirtualPath::new(path).unwrap(),
        ))
    }

    #[test]
    fn test_zip() {
        let template =
            read("../../assets/templates/table-0.1.0.zip").expect("Failed to read template zip");
        let files =
            PackedTemplate::new(Cursor::new(template)).expect("Failed to parse template zip");
        assert!(files.source(project_file("/main.typ")).is_ok());
    }

    #[test]
    fn recovers_from_poisoned_file_maps() {
        let template =
            read("../../assets/templates/table-0.1.0.zip").expect("Failed to read template zip");
        let files =
            PackedTemplate::new(Cursor::new(template)).expect("Failed to parse template zip");

        let _ = std::panic::catch_unwind(|| {
            let _source = files.source.lock().unwrap();
            let _bytes = files.bytes.lock().unwrap();
            panic!("poison the file maps");
        });
        assert!(files.source.is_poisoned());
        assert!(files.bytes.is_poisoned());

        assert!(files.source(project_file("/main.typ")).is_ok());
        assert!(files
            .file(package_file(
                "@preview/oicana:0.2.0".parse().unwrap(),
                "/typst.toml"
            ))
            .is_ok());
    }

    #[test]
    fn names_packages_missing_from_the_archive() {
        let template =
            read("../../assets/templates/table-0.1.0.zip").expect("Failed to read template zip");
        let files =
            PackedTemplate::new(Cursor::new(template)).expect("Failed to parse template zip");

        let missing_package = package_file("@preview/missing:1.0.0".parse().unwrap(), "/lib.typ");
        let error = files.source(missing_package).unwrap_err().to_string();
        assert!(
            error.contains("@preview/missing:1.0.0 is not part of the template archive"),
            "{error}"
        );

        let packed_package = package_file("@preview/oicana:0.2.0".parse().unwrap(), "/missing.typ");
        assert!(matches!(
            files.file(packed_package),
            Err(FileError::NotFound(_))
        ));
        assert!(matches!(
            files.source(project_file("/missing.typ")),
            Err(FileError::NotFound(_))
        ));
    }

    #[test]
    fn can_read_manifest() {
        let template =
            read("../../assets/templates/table-0.1.0.zip").expect("Failed to read template zip");
        let files =
            PackedTemplate::new(Cursor::new(template)).expect("Failed to parse template zip");
        let manifest = files
            .source(project_file("/typst.toml"))
            .expect("Failed to find typst.toml");

        let manifest: PackageManifest =
            toml::from_str(manifest.text()).expect("Failed to parse the manifest");
        assert!(manifest
            .tool
            .sections
            .contains_key(&EcoString::from("oicana")));
    }

    #[test]
    fn can_find_dependency() {
        let template =
            read("../../assets/templates/table-0.1.0.zip").expect("Failed to read template zip");
        let files =
            PackedTemplate::new(Cursor::new(template)).expect("Failed to parse template zip");

        assert!(files
            .file(package_file(
                "@preview/oicana:0.2.0".parse().unwrap(),
                "/typst.toml"
            ))
            .is_ok());
    }

    #[test]
    fn finds_fonts() {
        let template =
            read("../../assets/templates/fonts-0.1.0.zip").expect("Failed to read template zip");
        let files =
            PackedTemplate::new(Cursor::new(template)).expect("Failed to parse template zip");

        let mut found: Vec<String> = files
            .fonts
            .iter()
            .map(|id| id.vpath().get_with_slash().to_owned())
            .collect();
        found.sort();
        assert_eq!(
            found,
            vec![
                "/fonts/InriaSerif-Regular.ttf".to_owned(),
                "/fonts/NotoSansArabic-Regular.ttf".to_owned(),
            ]
        )
    }

    fn zip_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
        zip_with_compression(entries, CompressionMethod::Zstd)
    }

    fn zip_with_compression(entries: &[(&str, &[u8])], method: CompressionMethod) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(method);
        for (name, content) in entries {
            writer.start_file(*name, options).unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn reads_deflate_archive() {
        let archive = zip_with_compression(
            &[("main.typ", b"Hello".as_slice())],
            CompressionMethod::Deflated,
        );

        let files = PackedTemplate::new(Cursor::new(archive)).expect("Failed to read archive");

        assert_eq!(
            files.source(project_file("/main.typ")).unwrap().text(),
            "Hello"
        );
    }

    #[test]
    fn rejects_unsupported_compression() {
        let mut archive = zip_with_compression(
            &[("main.typ", b"Hello".as_slice())],
            CompressionMethod::Stored,
        );
        for (signature, method_offset) in [(*b"PK\x03\x04", 8), (*b"PK\x01\x02", 10)] {
            let header = archive
                .windows(4)
                .position(|window| window == signature)
                .unwrap();
            archive[header + method_offset..header + method_offset + 2]
                .copy_from_slice(&14u16.to_le_bytes());
        }

        let result = PackedTemplate::new(Cursor::new(archive));

        assert!(
            matches!(
                result,
                Err(PackedTemplateError::UnsupportedCompression { ref path, method: 14 })
                    if path == "main.typ"
            ),
            "{:?}",
            result.err()
        );
    }

    #[test]
    fn rejects_archive_with_too_many_entries() {
        let archive = zip_with_entries(&[
            ("a.typ", b"a".as_slice()),
            ("b.typ", b"b".as_slice()),
            ("c.typ", b"c".as_slice()),
        ]);

        let result = PackedTemplate::new_with_limits(
            Cursor::new(archive),
            ZipLimits {
                max_entries: 2,
                ..ZipLimits::default()
            },
        );

        assert!(matches!(
            result,
            Err(PackedTemplateError::TooManyEntries { count: 3, limit: 2 })
        ));
    }

    #[test]
    fn rejects_archive_exceeding_the_decompressed_size_limit() {
        // Highly compressible content: the zip stays tiny, only the
        // decompressed size trips the limit (a zip bomb in miniature).
        let archive = zip_with_entries(&[("main.typ", vec![b' '; 1024 * 1024].as_slice())]);
        assert!(archive.len() < 16 * 1024);

        let result = PackedTemplate::new_with_limits(
            Cursor::new(archive),
            ZipLimits {
                max_total_decompressed_bytes: 64 * 1024,
                ..ZipLimits::default()
            },
        );

        assert!(matches!(
            result,
            Err(PackedTemplateError::TooLarge { limit }) if limit == 64 * 1024
        ));
    }

    #[test]
    fn size_limit_applies_to_the_sum_of_all_entries() {
        let content = vec![b' '; 40 * 1024];
        let archive =
            zip_with_entries(&[("a.typ", content.as_slice()), ("b.typ", content.as_slice())]);

        let result = PackedTemplate::new_with_limits(
            Cursor::new(archive),
            ZipLimits {
                max_total_decompressed_bytes: 64 * 1024,
                ..ZipLimits::default()
            },
        );

        assert!(matches!(result, Err(PackedTemplateError::TooLarge { .. })));
    }

    #[test]
    fn reads_archive_that_exactly_fits_the_limits() {
        let content = vec![b' '; 32 * 1024];
        let archive =
            zip_with_entries(&[("a.typ", content.as_slice()), ("b.typ", content.as_slice())]);

        let files = PackedTemplate::new_with_limits(
            Cursor::new(archive),
            ZipLimits {
                max_entries: 2,
                max_total_decompressed_bytes: 64 * 1024,
            },
        )
        .expect("archive within the limits should load");

        assert!(files.source(project_file("/a.typ")).is_ok());
        assert!(files.source(project_file("/b.typ")).is_ok());
    }

    #[test]
    fn check_declared_rejects_too_many_entries() {
        let archive = zip_with_entries(&[
            ("a.typ", b"a".as_slice()),
            ("b.typ", b"b".as_slice()),
            ("c.typ", b"c".as_slice()),
        ]);

        let result = ZipLimits {
            max_entries: 2,
            ..ZipLimits::default()
        }
        .check_declared(Cursor::new(archive));

        assert!(matches!(
            result,
            Err(PackedTemplateError::TooManyEntries { count: 3, limit: 2 })
        ));
    }

    #[test]
    fn check_declared_rejects_oversized_content() {
        let content = vec![b' '; 40 * 1024];
        let archive =
            zip_with_entries(&[("a.typ", content.as_slice()), ("b.typ", content.as_slice())]);

        let result = ZipLimits {
            max_total_decompressed_bytes: 64 * 1024,
            ..ZipLimits::default()
        }
        .check_declared(Cursor::new(archive));

        assert!(matches!(
            result,
            Err(PackedTemplateError::TooLarge { limit }) if limit == 64 * 1024
        ));
    }

    #[test]
    fn check_declared_accepts_archive_within_limits() {
        let template =
            read("../../assets/templates/table-0.1.0.zip").expect("Failed to read template zip");

        assert!(ZipLimits::default()
            .check_declared(Cursor::new(template))
            .is_ok());
    }

    #[test]
    fn cannot_access_files_outside_zip() {
        let template =
            read("../../assets/templates/table-0.1.0.zip").expect("Failed to read template zip");
        let files =
            PackedTemplate::new(Cursor::new(template)).expect("Failed to parse template zip");

        // Paths escaping the root cannot even be constructed
        assert!(VirtualPath::new("/../../etc/passwd").is_err());
        // Attempting to access a path that doesn't exist in the zip returns NotFound
        assert!(files.source(project_file("/nonexistent.typ")).is_err());
    }
}
