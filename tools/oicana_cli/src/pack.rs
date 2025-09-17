use crate::target::TargetArgs;
use anyhow::Context;
use clap::Args;
use log::info;
use oicana_files::native::{package_data_dir, NativeTemplate};
use oicana_files::TemplateFiles;
use oicana_template::package::package;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{create_dir_all, read_dir, remove_dir_all, File};
use std::path::Path;
use std::str::FromStr;
use typst::syntax::ast::ModuleImport;
use typst::syntax::package::PackageSpec;
use typst::syntax::{ast, FileId, VirtualPath};

#[derive(Debug, Args)]
pub struct PackArgs {
    #[clap(flatten)]
    target: TargetArgs,
    #[clap(short, long, help = "Output directory", default_value = ".")]
    out_dir: String,
    #[clap(
        short,
        long,
        help = "Name template for the artifacts",
        default_value = "{template}-{version}.zip"
    )]
    name: String,
}

#[rustfmt::skip]
pub const PACK_AFTER_HELP: &str = color_print::cstr!("\
<s><u>Examples:</></>
  oicana pack templates/invoice
  oicana pack templates/invoice -o out
  oicana pack -a
  oicana pack templates -a
");

pub fn pack(args: PackArgs) -> anyhow::Result<()> {
    // Todo: read `exclude` from manifest and default to exclude zip files and the output dir to better support single dir templates
    let templates = args.target.get_targets()?;
    let out = Path::new(&args.out_dir);
    let packages = package_data_dir().context("Failed to find data directory for packages")?;

    for template in templates {
        info!("Packing template '{}'.", template.manifest.package.name);
        template.manifest.validate()?;

        let mut files = NativeTemplate::new(&template.path, packages.clone());

        let out_file_path = out.join(
            args.name
                .replace("{template}", &template.manifest.package.name)
                .replace("{version}", &template.manifest.package.version.to_string()),
        );

        update_dependencies(&template.path, &mut files)?;

        create_dir_all(out)?;
        let mut out_file = File::create(out_file_path).context("Failed to create the zip file")?;

        package(&template.path, &mut out_file, &template.manifest)?;
    }

    Ok(())
}

/// This will remove the current `.dependencies` directory and
/// resolve all imports from the template, recreating `.dependencies`
/// along the way.
fn update_dependencies<Files: TemplateFiles>(root: &Path, files: &mut Files) -> anyhow::Result<()> {
    let _ignore = remove_dir_all(root.join(".dependencies"));
    let mut imported_dependencies = HashSet::new();

    fn prepare_dependencies<Files: TemplateFiles>(
        root: &Path,
        dir: &Path,
        files: &mut Files,
        imported_dependencies: &mut HashSet<PackageSpec>,
    ) -> anyhow::Result<()> {
        if dir.file_name().and_then(OsStr::to_str) == Some(".dependencies") {
            return Ok(());
        }
        for entry in read_dir(dir).context("Failed to read directory")? {
            let entry = entry?;
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            let path = dir.join(entry.file_name());
            if meta.is_dir() {
                prepare_dependencies(root, &path, files, imported_dependencies)?;
            }
            if path.extension().and_then(|ext| ext.to_str()) == Some("typ") {
                let fid = FileId::new(
                    None,
                    VirtualPath::new(path.strip_prefix(root).context(
                        "Prefix striping failed even though `path` is built from `root`",
                    )?),
                );
                let source = files.source(fid).context("Can't read source file")?;
                let imports = source
                    .root()
                    .children()
                    .filter_map(|ch| ch.cast::<ModuleImport>());
                for import in imports {
                    let ast::Expr::Str(source_str) = import.source() else {
                        continue;
                    };

                    if let Ok(import_spec) = PackageSpec::from_str(source_str.get().as_str()) {
                        let package_file_id =
                            FileId::new(Some(import_spec.clone()), VirtualPath::new("/typst.toml"));
                        // Request the package manifest file. This will be cached in case we already prepared the dependency.
                        // Otherwise the package will be copied from the local machine's Typst cache or downloaded from the registry.
                        files.file(package_file_id).context(format!(
                            "Failed to prepare package for file {package_file_id:?}"
                        ))?;
                        if imported_dependencies.insert(import_spec.clone()) {
                            prepare_dependencies(
                                root,
                                &root
                                    .join(".dependencies")
                                    .join(import_spec.namespace.to_string())
                                    .join(import_spec.name.to_string())
                                    .join(import_spec.version.to_string()),
                                files,
                                imported_dependencies,
                            )?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    prepare_dependencies(root, root, files, &mut imported_dependencies)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oicana_files::{native::NativeTemplate, TemplateFiles};
    use std::{
        collections::{HashMap, HashSet},
        fs::create_dir_all,
        io::Write,
        path::PathBuf,
        sync::RwLock,
    };
    use tempfile::tempdir;
    use typst::{foundations::Bytes, syntax::package::PackageSpec};

    struct TestFiles {
        native: NativeTemplate,
        mocked_packages: HashMap<PackageSpec, String>,
        packages: RwLock<HashSet<PackageSpec>>,
        root: PathBuf,
    }

    impl TemplateFiles for TestFiles {
        fn source(
            &self,
            id: typst::syntax::FileId,
        ) -> typst::diag::FileResult<typst::syntax::Source> {
            self.native.source(id)
        }

        // We prevent network calls or checks in the local Typst cache here.
        fn file(
            &self,
            id: typst::syntax::FileId,
        ) -> typst::diag::FileResult<typst::foundations::Bytes> {
            if let Some(spec) = id.package() {
                self.packages.write().unwrap().insert(spec.clone());
                let package = self.mocked_packages.get(spec).unwrap();
                let subdir = format!(
                    ".dependencies/{}/{}/{}",
                    spec.namespace, spec.name, spec.version
                );
                create_dir_all(self.root.join(&subdir)).unwrap();
                let mut tmp_file =
                    File::create(self.root.join(&subdir).join("package.typ")).unwrap();
                tmp_file.write_all(package.as_bytes()).unwrap();

                return Ok(Bytes::from_string(package.clone()));
            }

            self.native.file(id)
        }

        fn font_files(&self) -> &Vec<typst::syntax::FileId> {
            todo!()
        }
    }

    #[test]
    fn no_dependency_dir_without_dependencies() {
        let tempdir = tempdir().unwrap();
        let temp_template = tempdir.path().join("template");
        create_dir_all(&temp_template).unwrap();
        let temp_packages = tempdir.path().join("cache");
        {
            let file_path = temp_template.join("test.typ");
            let mut tmp_file = File::create(file_path).unwrap();
            tmp_file
                .write_all("This Typst file has no imports!".as_bytes())
                .unwrap();
        }
        let mut files = TestFiles {
            native: NativeTemplate::new(&temp_template, temp_packages),
            mocked_packages: HashMap::new(),
            packages: RwLock::new(HashSet::new()),
            root: temp_template.to_path_buf(),
        };

        update_dependencies(&temp_template, &mut files).unwrap();
        assert!(files.packages.read().unwrap().is_empty());
        assert_eq!(
            temp_template.join(".dependencies").try_exists().ok(),
            Some(false)
        );
    }

    #[test]
    fn resolves_dependencies() {
        let tempdir = tempdir().unwrap();
        let temp_template = tempdir.path().join("template");
        create_dir_all(&temp_template).unwrap();
        let temp_packages = tempdir.path().join("cache");
        {
            let file_path = temp_template.join("test.typ");
            let mut tmp_file = File::create(file_path).unwrap();
            tmp_file
                .write_all(
                    "#import \"@preview/test:0.1.0\": *\nThis Typst file imports the test package."
                        .as_bytes(),
                )
                .unwrap();
        }
        let root = temp_template.to_path_buf();
        let spec = PackageSpec::from_str("@preview/test:0.1.0").unwrap();
        let mut mocked_packages = HashMap::new();
        mocked_packages.insert(spec.clone(), "Some package content".to_owned());
        let mut files = TestFiles {
            native: NativeTemplate::new(&temp_template, temp_packages),
            mocked_packages,
            packages: RwLock::new(HashSet::new()),
            root: root.clone(),
        };
        update_dependencies(&temp_template, &mut files).unwrap();
        assert_eq!(files.packages.read().unwrap().len(), 1);
        assert!(files.packages.read().unwrap().get(&spec).is_some());
        assert_eq!(
            temp_template.join(".dependencies").try_exists().ok(),
            Some(true)
        );
    }

    #[test]
    fn resolves_imports_in_dependencies() {
        let tempdir = tempdir().unwrap();
        let temp_template = tempdir.path().join("template");
        create_dir_all(&temp_template).unwrap();
        let temp_packages = tempdir.path().join("cache");
        {
            let file_path = temp_template.join("test.typ");
            let mut tmp_file = File::create(file_path).unwrap();
            tmp_file
                .write_all(
                    "#import \"@preview/test:0.1.0\": *\nThis Typst file imports the test package."
                        .as_bytes(),
                )
                .unwrap();
        }
        let root = temp_template.to_path_buf();
        let spec = PackageSpec::from_str("@preview/test:0.1.0").unwrap();
        let spec2 = PackageSpec::from_str("@preview/test2:0.1.0").unwrap();
        let mut mocked_packages = HashMap::new();
        mocked_packages.insert(
            spec.clone(),
            "#import \"@preview/test2:0.1.0\": *\nSome package content with import".to_owned(),
        );
        mocked_packages.insert(spec2.clone(), "Some other package content".to_owned());
        let mut files = TestFiles {
            native: NativeTemplate::new(&temp_template, temp_packages),
            mocked_packages,
            packages: RwLock::new(HashSet::new()),
            root: root.clone(),
        };
        update_dependencies(&temp_template, &mut files).unwrap();
        assert_eq!(files.packages.read().unwrap().len(), 2);
        assert!(files.packages.read().unwrap().get(&spec).is_some());
        assert!(files.packages.read().unwrap().get(&spec2).is_some());
        assert_eq!(
            temp_template.join(".dependencies").try_exists().ok(),
            Some(true)
        );
    }
}
