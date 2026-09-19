use crate::target::TargetArgs;
use anyhow::Context;
use clap::Args;
use console::{style, Emoji};
use log::info;
use oicana::files::native::{package_data_dir, NativeTemplate};
use oicana::files::packed::ZipLimits;
use oicana::files::TemplateFiles;
use oicana::template::package::package_with_dependencies;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{create_dir_all, File};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use typst::syntax::ast::{ModuleImport, ModuleInclude};

static PACKAGE: Emoji<'_, '_> = Emoji("📦", "");
use typst::syntax::package::PackageSpec;
use typst::syntax::{ast, FileId, RootedPath, SyntaxNode, VirtualPath, VirtualRoot};

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
    let templates = args.target.get_targets()?;
    let out = Path::new(&args.out_dir);
    let packages = package_data_dir().context("Failed to find data directory for packages")?;

    for template in templates {
        info!("Packing template '{}'.", template.manifest.package.name);
        template.manifest.validate_at(&template.path)?;

        let files = NativeTemplate::new(&template.path, packages.clone());

        let dependencies = collect_dependencies(&template.path, &files)?;

        create_dir_all(out)?;
        let out_file_path = out.join(
            args.name
                .replace("{template}", &template.manifest.package.name)
                .replace("{version}", &template.manifest.package.version.to_string()),
        );
        let out_file = File::create(&out_file_path).context("Failed to create the zip file")?;

        // Otherwise the zip file includes a partial version of itself if `pack` is called in the template directory
        let exclude = out_file_path.canonicalize().ok().and_then(|abs_out| {
            template.path.canonicalize().ok().and_then(|abs_template| {
                abs_out
                    .strip_prefix(abs_template)
                    .ok()
                    .map(Path::to_path_buf)
            })
        });

        let exclude_matcher = template.manifest.build_exclude_matcher();
        package_with_dependencies(
            &template.path,
            out_file,
            &exclude_matcher,
            exclude.as_deref(),
            &dependencies,
        )?;

        println!(
            "{PACKAGE}  {} packed to {}",
            style(&template.manifest.package.name).bold(),
            style(out_file_path.display()).cyan(),
        );

        warn_if_template_exceeds_default_limits(&out_file_path);
    }

    Ok(())
}

/// Warn if the packed template exceeds the default archive limits of the integrations.
///
/// Registering such a template will fail unless Oicana is configured
/// with higher archive limits.
fn warn_if_template_exceeds_default_limits(out_file_path: &Path) {
    let Ok(packed) = File::open(out_file_path) else {
        return;
    };
    if let Err(error) = ZipLimits::default().check_declared(packed) {
        let warning = style("Warning").yellow();
        println!("{warning}: {error}.");
        println!("   Oicana integrations with default limits will reject this template.");
    }
}

/// Collect all package dependencies by scanning imports in template `.typ` files.
///
/// Returns a list of `(source_dir, zip_prefix)` pairs where `source_dir` is the
/// package's location on disk and `zip_prefix` is the path it should have in the zip
/// (e.g. `.dependencies/preview/pkg/0.1.0`).
fn collect_dependencies(
    root: &Path,
    files: &NativeTemplate,
) -> anyhow::Result<Vec<(PathBuf, PathBuf)>> {
    let mut collected = HashSet::new();
    let mut result = Vec::new();

    scan_imports(
        root,
        &VirtualRoot::Project,
        files,
        &mut collected,
        &mut result,
    )?;

    Ok(result)
}

fn scan_imports(
    dir: &Path,
    virtual_root: &VirtualRoot,
    files: &NativeTemplate,
    collected: &mut HashSet<PackageSpec>,
    result: &mut Vec<(PathBuf, PathBuf)>,
) -> anyhow::Result<()> {
    let walk = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_entry(|entry| entry.file_name() != OsStr::new(".dependencies"));
    for entry in walk {
        let entry = entry.context("Failed to read the directory to scan for imports")?;
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().and_then(OsStr::to_str) != Some("typ") {
            continue;
        }

        let vpath = VirtualPath::virtualize(dir, path)
            .context("Path virtualization failed even though `path` is built from `dir`")?;
        let fid = FileId::new(RootedPath::new(virtual_root.clone(), vpath));
        let source = files
            .source(fid)
            .context(format!("Can't read source file {}", path.display()))?;

        for literal in imported_literals(source.root()) {
            let Ok(spec) = PackageSpec::from_str(literal.as_str()) else {
                continue;
            };
            if !collected.insert(spec.clone()) {
                continue;
            }

            let package_dir = files.package_dir(&spec).context(format!(
                "Failed to resolve package {spec} imported by {}",
                path.display()
            ))?;
            let zip_prefix = PathBuf::from(format!(
                ".dependencies/{}/{}/{}",
                spec.namespace, spec.name, spec.version
            ));
            result.push((package_dir.clone(), zip_prefix));

            scan_imports(
                &package_dir,
                &VirtualRoot::Package(spec),
                files,
                collected,
                result,
            )?;
        }
    }

    Ok(())
}

fn imported_literals(node: &SyntaxNode) -> Vec<String> {
    let mut found = Vec::new();
    collect_imported_literals(node, &mut found);
    found
}

fn collect_imported_literals(node: &SyntaxNode, found: &mut Vec<String>) {
    let source = node
        .cast::<ModuleImport>()
        .map(|import| import.source())
        .or_else(|| node.cast::<ModuleInclude>().map(|include| include.source()));
    if let Some(ast::Expr::Str(literal)) = source {
        found.push(literal.get().to_string());
    }
    for child in node.children() {
        collect_imported_literals(child, found);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::fs::create_dir_all;
    use std::io::Write;
    use tempfile::tempdir;

    fn create_mock_package(packages_dir: &Path, spec: &PackageSpec, content: &str) {
        let package_dir =
            packages_dir.join(format!("{}/{}/{}", spec.namespace, spec.name, spec.version));
        create_dir_all(&package_dir).unwrap();
        let mut f = File::create(package_dir.join("package.typ")).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        // Also create a typst.toml so the package is valid
        let mut m = File::create(package_dir.join("typst.toml")).unwrap();
        m.write_all(
            format!(
                r#"[package]
name = "{}"
version = "{}"
entrypoint = "package.typ"
"#,
                spec.name, spec.version
            )
            .as_bytes(),
        )
        .unwrap();
    }

    #[test]
    fn no_dependencies_for_template_without_imports() {
        let tempdir = tempdir().unwrap();
        let temp_template = tempdir.path().join("template");
        create_dir_all(&temp_template).unwrap();
        let temp_packages = tempdir.path().join("cache");
        create_dir_all(&temp_packages).unwrap();
        {
            let file_path = temp_template.join("test.typ");
            let mut tmp_file = File::create(file_path).unwrap();
            tmp_file
                .write_all("This Typst file has no imports!".as_bytes())
                .unwrap();
        }
        let files = NativeTemplate::new(&temp_template, temp_packages);

        let deps = collect_dependencies(&temp_template, &files).unwrap();
        assert!(deps.is_empty());
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
                    "#import \"@local/test:0.1.0\": *\nThis Typst file imports the test package."
                        .as_bytes(),
                )
                .unwrap();
        }
        let spec = PackageSpec::from_str("@local/test:0.1.0").unwrap();
        create_mock_package(&temp_packages, &spec, "Some package content");

        let files = NativeTemplate::new(&temp_template, temp_packages);
        let deps = collect_dependencies(&temp_template, &files).unwrap();

        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].1, PathBuf::from(".dependencies/local/test/0.1.0"));
        // No .dependencies created on disk
        assert_eq!(
            temp_template.join(".dependencies").try_exists().ok(),
            Some(false)
        );
    }

    #[test]
    fn collects_a_package_once_even_when_imported_repeatedly() {
        let tempdir = tempdir().unwrap();
        let temp_template = tempdir.path().join("template");
        create_dir_all(temp_template.join("sub")).unwrap();
        let temp_packages = tempdir.path().join("cache");

        for file in ["a.typ", "b.typ", "sub/c.typ"] {
            File::create(temp_template.join(file))
                .unwrap()
                .write_all(b"#import \"@local/test:0.1.0\": *")
                .unwrap();
        }
        let spec = PackageSpec::from_str("@local/test:0.1.0").unwrap();
        create_mock_package(&temp_packages, &spec, "Some package content");

        let files = NativeTemplate::new(&temp_template, temp_packages);
        let deps = collect_dependencies(&temp_template, &files).unwrap();

        assert_eq!(deps.len(), 1, "the package should be packed once: {deps:?}");
    }

    #[test]
    fn terminates_on_cyclic_package_dependencies() {
        let tempdir = tempdir().unwrap();
        let temp_template = tempdir.path().join("template");
        create_dir_all(&temp_template).unwrap();
        let temp_packages = tempdir.path().join("cache");

        File::create(temp_template.join("test.typ"))
            .unwrap()
            .write_all(b"#import \"@local/first:0.1.0\": *")
            .unwrap();

        let first = PackageSpec::from_str("@local/first:0.1.0").unwrap();
        let second = PackageSpec::from_str("@local/second:0.1.0").unwrap();
        create_mock_package(&temp_packages, &first, "#import \"@local/second:0.1.0\": *");
        create_mock_package(&temp_packages, &second, "#import \"@local/first:0.1.0\": *");

        let files = NativeTemplate::new(&temp_template, temp_packages);
        let deps = collect_dependencies(&temp_template, &files).unwrap();

        assert_eq!(deps.len(), 2, "both packages exactly once: {deps:?}");
    }

    #[test]
    fn resolves_nested_imports_and_includes() {
        for markup in [
            "#let helper() = {\n  import \"@local/test:0.1.0\": *\n  [hi]\n}",
            "#{\n  import \"@local/test:0.1.0\": *\n}",
            "#if true {\n  import \"@local/test:0.1.0\": *\n}",
            "#include \"@local/test:0.1.0\"",
            "#{\n  include \"@local/test:0.1.0\"\n}",
        ] {
            let tempdir = tempdir().unwrap();
            let temp_template = tempdir.path().join("template");
            create_dir_all(&temp_template).unwrap();
            let temp_packages = tempdir.path().join("cache");
            File::create(temp_template.join("test.typ"))
                .unwrap()
                .write_all(markup.as_bytes())
                .unwrap();

            let spec = PackageSpec::from_str("@local/test:0.1.0").unwrap();
            create_mock_package(&temp_packages, &spec, "Some package content");

            let files = NativeTemplate::new(&temp_template, temp_packages);
            let deps = collect_dependencies(&temp_template, &files).unwrap();

            assert_eq!(deps.len(), 1, "expected a dependency for {markup:?}");
            assert_eq!(deps[0].1, PathBuf::from(".dependencies/local/test/0.1.0"));
        }
    }

    #[test]
    fn resolves_transitive_dependencies() {
        let tempdir = tempdir().unwrap();
        let temp_template = tempdir.path().join("template");
        create_dir_all(&temp_template).unwrap();
        let temp_packages = tempdir.path().join("cache");
        {
            let file_path = temp_template.join("test.typ");
            let mut tmp_file = File::create(file_path).unwrap();
            tmp_file
                .write_all(
                    "#import \"@local/test:0.1.0\": *\nThis Typst file imports the test package."
                        .as_bytes(),
                )
                .unwrap();
        }
        let spec = PackageSpec::from_str("@local/test:0.1.0").unwrap();
        let spec2 = PackageSpec::from_str("@local/test2:0.1.0").unwrap();
        create_mock_package(
            &temp_packages,
            &spec,
            "#import \"@local/test2:0.1.0\": *\nSome package content with import",
        );
        create_mock_package(&temp_packages, &spec2, "Some other package content");

        let files = NativeTemplate::new(&temp_template, temp_packages);
        let deps = collect_dependencies(&temp_template, &files).unwrap();

        assert_eq!(deps.len(), 2);
        let zip_prefixes: HashSet<_> = deps.iter().map(|(_, p)| p.clone()).collect();
        assert!(zip_prefixes.contains(&PathBuf::from(".dependencies/local/test/0.1.0")));
        assert!(zip_prefixes.contains(&PathBuf::from(".dependencies/local/test2/0.1.0")));
        // No .dependencies created on disk
        assert_eq!(
            temp_template.join(".dependencies").try_exists().ok(),
            Some(false)
        );
    }
}
