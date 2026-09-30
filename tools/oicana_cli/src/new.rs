use anyhow::{bail, Context};
use clap::Args;
use color_print::cstr;
use oicana::template::manifest::is_valid_template_name;
use std::fs;
use std::path::{Path, PathBuf};
use typst::syntax::package::PackageVersion;

#[rustfmt::skip]
pub const NEW_AFTER_HELP: &str = cstr!("\
<s><u>Examples:</></>
  oicana new my-template
  oicana new invoice --version 1.0.0
");

#[derive(Debug, Args)]
pub struct NewArgs {
    /// Name of the new template (also the directory and Typst package name).
    name: String,
    /// Initial template version.
    #[arg(long, default_value = "0.1.0", value_parser = parse_version)]
    version: PackageVersion,
}

fn parse_version(version: &str) -> Result<PackageVersion, String> {
    version.parse().map_err(|error| format!("{error}"))
}

const MAIN_FILE: &str = r#"#set document(date: datetime.today())

= Hello

Edit `main.typ` to design your template.
"#;

pub fn new(args: NewArgs) -> anyhow::Result<()> {
    let dir = PathBuf::from(&args.name);
    create_template(&dir, &args.name, &args.version.to_string())?;
    println!(
        "Created Oicana template '{}' in ./{}/",
        args.name, args.name
    );
    Ok(())
}

fn create_template(dir: &Path, name: &str, version: &str) -> anyhow::Result<()> {
    if !is_valid_template_name(name) {
        bail!(
            "Template name '{}' is not a valid identifier. Use letters, digits, '_' or '-'; must start with a letter or '_'.",
            name
        );
    }

    if dir.exists() {
        bail!("'{}' already exists", dir.display());
    }

    fs::create_dir(dir)
        .with_context(|| format!("Failed to create directory '{}'", dir.display()))?;

    let typst_toml = format!(
        r#"[package]
name = "{name}"
version = "{version}"
entrypoint = "main.typ"

[tool.oicana]
manifest_version = 1

[tool.oicana.export.pdf]
standards = ["a-3b"]
"#,
    );

    fs::write(dir.join("typst.toml"), typst_toml).context("Failed to write typst.toml")?;
    fs::write(dir.join("main.typ"), MAIN_FILE).context("Failed to write main.typ")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn creates_typst_toml_and_main_typ() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("my-template");
        create_template(&dir, "my-template", "0.1.0").unwrap();

        let toml = fs::read_to_string(dir.join("typst.toml")).unwrap();
        assert!(toml.contains(r#"name = "my-template""#));
        assert!(toml.contains(r#"version = "0.1.0""#));
        assert!(toml.contains(r#"entrypoint = "main.typ""#));
        assert!(toml.contains("[tool.oicana]"));
        assert!(toml.contains("manifest_version = 1"));
        assert!(toml.contains(r#"standards = ["a-3b"]"#));

        let main = fs::read_to_string(dir.join("main.typ")).unwrap();
        assert!(main.contains("Hello"));
        assert!(main.contains("Edit `main.typ`"));
    }

    #[test]
    fn honors_custom_version() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("invoice");
        create_template(&dir, "invoice", "1.2.3").unwrap();
        let toml = fs::read_to_string(dir.join("typst.toml")).unwrap();
        assert!(toml.contains(r#"version = "1.2.3""#));
    }

    #[test]
    fn rejects_invalid_name() {
        let tmp = tempdir().unwrap();
        let err = create_template(&tmp.path().join("bad"), "My Template", "0.1.0").unwrap_err();
        assert!(err.to_string().contains("not a valid identifier"));
    }

    #[test]
    fn refuses_to_clobber_existing_directory() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("conflict");
        fs::create_dir(&dir).unwrap();
        let err = create_template(&dir, "conflict", "0.1.0").unwrap_err();
        assert!(err.to_string().contains("already exists"));
    }

    #[test]
    fn generated_template_passes_validate() {
        let tmp = tempdir().unwrap();
        let dir = tmp.path().join("scaffold-test");
        create_template(&dir, "scaffold-test", "0.1.0").unwrap();
        oicana::template::validate_native_template(&dir).unwrap();
    }
}
