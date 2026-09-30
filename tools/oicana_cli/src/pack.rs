use crate::checks::{check_entrypoint, check_pdf_export, check_schemas, SchemaSelection};
use crate::target::TargetArgs;
use anyhow::Context;
use clap::Args;
use console::{style, Emoji};
use ignore::gitignore::Gitignore;
use log::info;
use oicana::files::native::{package_data_dir, NativeTemplate};
use oicana::files::packed::ZipLimits;
use oicana::files::TemplateFiles;
use oicana::template::package::{dependency_exclude_matcher, package_with_dependencies};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::{create_dir_all, File};
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use typst::syntax::ast::{AstNode, ModuleImport, ModuleInclude};

static PACKAGE: Emoji<'_, '_> = Emoji("📦", "");
use typst::syntax::package::PackageSpec;
use typst::syntax::{ast, FileId, LinkedNode, RootedPath, VirtualPath, VirtualRoot};

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
    let templates = args.target.get_targets()?.templates;
    let out = Path::new(&args.out_dir);
    let packages = package_data_dir().context("Failed to find data directory for packages")?;

    for template in templates {
        info!("Packing template '{}'.", template.manifest.package.name);
        template.manifest.validate_at(&template.path)?;

        let mut errors: Vec<String> = check_entrypoint(&template.path, &template.manifest)
            .err()
            .into_iter()
            .collect();
        errors.extend(
            check_schemas(
                &template.path,
                &template.manifest,
                SchemaSelection::Validated,
            )
            .errors,
        );
        errors.extend(check_pdf_export(&template.manifest));
        if !errors.is_empty() {
            anyhow::bail!(
                "Template '{}' has fatal issues:\n  - {}",
                template.manifest.package.name,
                errors.join("\n  - ")
            );
        }

        let files = NativeTemplate::new(&template.path, packages.clone());

        let exclude_matcher = template.manifest.build_exclude_matcher();
        let dependencies = collect_dependencies(&template.path, &exclude_matcher, &files)?;
        for dynamic_import in &dependencies.dynamic_imports {
            let warning = style("Warning").yellow();
            println!(
                "{warning}: {dynamic_import} is not a string literal. A package imported this way is not packed and fails at runtime."
            );
        }

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

        package_with_dependencies(
            &template.path,
            out_file,
            &exclude_matcher,
            exclude.as_deref(),
            &dependencies.packages,
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

/// Package dependencies of a template.
struct Dependencies {
    /// `(source_dir, zip_prefix)` pairs.
    packages: Vec<(PathBuf, PathBuf)>,
    /// Imports and includes whose source is computed, so they cannot be resolved
    /// before compilation.
    dynamic_imports: Vec<String>,
}

/// Collect all package dependencies by scanning imports in (to be packed) `.typ` files.
fn collect_dependencies(
    root: &Path,
    exclude_matcher: &Gitignore,
    files: &NativeTemplate,
) -> anyhow::Result<Dependencies> {
    let mut collected = HashSet::new();
    let mut dependencies = Dependencies {
        packages: Vec::new(),
        dynamic_imports: Vec::new(),
    };

    scan_imports(
        root,
        exclude_matcher,
        &VirtualRoot::Project,
        files,
        &mut collected,
        &mut dependencies,
    )?;

    Ok(dependencies)
}

fn scan_imports(
    dir: &Path,
    exclude_matcher: &Gitignore,
    virtual_root: &VirtualRoot,
    files: &NativeTemplate,
    collected: &mut HashSet<PackageSpec>,
    dependencies: &mut Dependencies,
) -> anyhow::Result<()> {
    let walk = walkdir::WalkDir::new(dir)
        .into_iter()
        .filter_entry(|entry| {
            let relative = entry.path().strip_prefix(dir).unwrap_or(entry.path());
            entry.file_name() != OsStr::new(".dependencies")
                && !exclude_matcher
                    .matched_path_or_any_parents(relative, entry.file_type().is_dir())
                    .is_ignore()
        });
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

        let mut sources = Vec::new();
        collect_import_sources(&LinkedNode::new(source.root()), &mut sources);
        for import_source in sources {
            let literal = match import_source {
                ImportSource::Literal(literal) => literal,
                ImportSource::Dynamic(range) => {
                    let line = source
                        .lines()
                        .byte_to_line(range.start)
                        .map_or(String::new(), |line| format!(":{}", line + 1));
                    dependencies.dynamic_imports.push(format!(
                        "The import source `{}` in {}{line}",
                        &source.text()[range],
                        path.display()
                    ));
                    continue;
                }
            };
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
            dependencies
                .packages
                .push((package_dir.clone(), zip_prefix));

            scan_imports(
                &package_dir,
                &dependency_exclude_matcher(&package_dir),
                &VirtualRoot::Package(spec),
                files,
                collected,
                dependencies,
            )?;
        }
    }

    Ok(())
}

/// The source of an `import` or `include`.
enum ImportSource {
    Literal(String),
    /// A computed source, with its byte range in the file.
    Dynamic(Range<usize>),
}

fn collect_import_sources(node: &LinkedNode, found: &mut Vec<ImportSource>) {
    let source = node
        .cast::<ModuleImport>()
        .map(|import| import.source())
        .or_else(|| node.cast::<ModuleInclude>().map(|include| include.source()));
    match source {
        Some(ast::Expr::Str(literal)) => {
            found.push(ImportSource::Literal(literal.get().to_string()));
        }
        Some(expression) => {
            let range = node
                .children()
                .find(|child| child.span() == expression.span())
                .map_or(node.range(), |child| child.range());
            found.push(ImportSource::Dynamic(range));
        }
        None => {}
    }
    for child in node.children() {
        collect_import_sources(&child, found);
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

        let deps = collect_dependencies(&temp_template, &Gitignore::empty(), &files)
            .unwrap()
            .packages;
        assert!(deps.is_empty());
        assert_eq!(
            temp_template.join(".dependencies").try_exists().ok(),
            Some(false)
        );
    }

    #[test]
    fn skips_imports_in_excluded_files() {
        let tempdir = tempdir().unwrap();
        let temp_template = tempdir.path().join("template");
        create_dir_all(temp_template.join("drafts")).unwrap();
        let temp_packages = tempdir.path().join("cache");
        create_dir_all(&temp_packages).unwrap();
        std::fs::write(
            temp_template.join("drafts/old.typ"),
            "#import \"@local/missing:0.1.0\": *",
        )
        .unwrap();

        let mut builder = ignore::gitignore::GitignoreBuilder::new("");
        builder.add_line(None, "drafts/").unwrap();
        let files = NativeTemplate::new(&temp_template, temp_packages);
        let deps = collect_dependencies(&temp_template, &builder.build().unwrap(), &files)
            .unwrap()
            .packages;

        assert!(deps.is_empty());
    }

    #[test]
    fn skips_imports_in_files_a_dependency_excludes() {
        let tempdir = tempdir().unwrap();
        let temp_template = tempdir.path().join("template");
        create_dir_all(&temp_template).unwrap();
        let temp_packages = tempdir.path().join("cache");
        std::fs::write(
            temp_template.join("test.typ"),
            "#import \"@local/test:0.1.0\": *",
        )
        .unwrap();
        let spec = PackageSpec::from_str("@local/test:0.1.0").unwrap();
        create_mock_package(&temp_packages, &spec, "Some package content");
        let package_dir = temp_packages.join("local/test/0.1.0");
        create_dir_all(package_dir.join("examples")).unwrap();
        std::fs::write(
            package_dir.join("examples/demo.typ"),
            "#import \"@local/missing:0.1.0\": *",
        )
        .unwrap();
        let mut manifest = std::fs::OpenOptions::new()
            .append(true)
            .open(package_dir.join("typst.toml"))
            .unwrap();
        manifest.write_all(b"exclude = [\"examples\"]\n").unwrap();

        let files = NativeTemplate::new(&temp_template, temp_packages);
        let deps = collect_dependencies(&temp_template, &Gitignore::empty(), &files)
            .unwrap()
            .packages;

        assert_eq!(deps.len(), 1);
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
        let deps = collect_dependencies(&temp_template, &Gitignore::empty(), &files)
            .unwrap()
            .packages;

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
        let deps = collect_dependencies(&temp_template, &Gitignore::empty(), &files)
            .unwrap()
            .packages;

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
        let deps = collect_dependencies(&temp_template, &Gitignore::empty(), &files)
            .unwrap()
            .packages;

        assert_eq!(deps.len(), 2, "both packages exactly once: {deps:?}");
    }

    #[test]
    fn reports_imports_with_a_computed_source() {
        let tempdir = tempdir().unwrap();
        let temp_template = tempdir.path().join("template");
        create_dir_all(&temp_template).unwrap();
        File::create(temp_template.join("test.typ"))
            .unwrap()
            .write_all(
                b"#let p = \"@local/\" + \"test:0.1.0\"\n#import p: *\n#import \"other.typ\"",
            )
            .unwrap();

        let files = NativeTemplate::new(&temp_template, tempdir.path().join("cache"));
        let dependencies =
            collect_dependencies(&temp_template, &Gitignore::empty(), &files).unwrap();

        assert!(dependencies.packages.is_empty());
        assert_eq!(dependencies.dynamic_imports.len(), 1);
        let reported = &dependencies.dynamic_imports[0];
        assert!(reported.contains("`p`"), "{reported}");
        assert!(reported.ends_with("test.typ:2"), "{reported}");
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
            let deps = collect_dependencies(&temp_template, &Gitignore::empty(), &files)
                .unwrap()
                .packages;

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
        let deps = collect_dependencies(&temp_template, &Gitignore::empty(), &files)
            .unwrap()
            .packages;

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
