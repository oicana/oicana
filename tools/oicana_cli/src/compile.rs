use crate::compile::export::{export_image, export_pdf, ExportFormat, ImageExportFormat};
use crate::fonts::FontArgs;
use anyhow::{anyhow, bail, Context, Ok};
use chrono::Utc;
use clap::Args;
use console::{style, Emoji};
use log::info;
use oicana::files::native::NativeTemplate;
use oicana::input::{BlobInput, JsonInput};
use oicana::input::{CompilationConfig, TemplateInputs};
use oicana::world::diagnostics::DiagnosticColor;
use oicana::Template;
use std::collections::HashMap;
use std::fs::{self, read, read_to_string};
use std::path::Path;

pub(crate) mod export;

pub(crate) static CHECKMARK: Emoji<'_, '_> = Emoji("✔️", "");

#[rustfmt::skip]
pub const COMPILE_AFTER_HELP: &str = color_print::cstr!("\
<s><u>Examples:</></>
  oicana compile
  oicana compile templates/invoice
  oicana compile -j test=inputs/input1.json -j foo=bar.json -b logo=company.png
  oicana compile --font-path /usr/share/fonts
");

#[derive(Debug, Args)]
pub struct CompileArgs {
    #[arg(
        help = "Path to the template. If not given, the current directory is expected to be a template."
    )]
    pub(crate) template: Option<String>,
    #[arg(short, long, help = "Output format", default_value = "pdf")]
    pub(crate) format: ExportFormat,
    #[clap(
        short,
        long,
        help = "Values for json inputs",
        value_name = "KEY=VALUE",
        num_args = 0..
    )]
    json: Vec<String>,
    #[arg(
        short,
        long,
        help = "Values for blob inputs",
        value_name = "KEY=VALUE",
        num_args = 0..
    )]
    blob: Vec<String>,
    #[arg(
        short = 'm',
        long,
        help = "Relative path to a json file to use as metadata for blob inputs.\nEvery blob metadata has to have a corresponding blob value.",
        value_name = "KEY=VALUE",
        num_args = 0..
    )]
    blob_meta: Vec<String>,
    #[arg(short, long, help = "Compile the template in development mode")]
    development: bool,
    #[clap(short, long, help = "Output directory", default_value = "./output")]
    pub(crate) out_dir: String,
    #[clap(
        short,
        long,
        help = "Name template for the artifacts. Available variables: {template}, {version}, {timestamp}, {format}",
        default_value = "{template}.{format}"
    )]
    pub(crate) name: String,
    #[arg(
        long,
        help = "PDF standards to enforce (e.g., 'a-3b', '2.0,a-4'). Overrides manifest settings.",
        value_name = "STANDARDS",
        value_delimiter = ','
    )]
    pub(crate) pdf_standards: Option<Vec<String>>,
    #[clap(flatten)]
    pub(crate) fonts: FontArgs,
}

pub fn compile(args: CompileArgs) -> anyhow::Result<()> {
    let inputs = build_inputs(&args)?;

    let path = match args.template {
        None => Path::new("."),
        Some(ref template) => Path::new(template),
    };
    let mut template = Template::open_with_fonts(path, &args.fonts.load())?;
    template.set_diagnostic_color(diagnostic_color());
    let name: String = template.manifest().package.name.to_string();
    info!("Compiling template '{name}'.");

    let result = template.compile(inputs)?;

    let document = result.document;
    if let Some(warnings) = result.warnings {
        println!("{warnings}");
    }

    let out_dir = Path::new(&args.out_dir);
    fs::create_dir_all(out_dir)?;

    let file_name = build_file_name(&args, &template);

    let out = out_dir.join(file_name);
    match args.format {
        ExportFormat::Pdf => export_pdf(&document, &out, &template, args.pdf_standards)?,
        ExportFormat::Png => export_image(&document, &out, ImageExportFormat::Png)?,
        ExportFormat::Svg => export_image(&document, &out, ImageExportFormat::Svg)?,
    }

    println!(
        "{CHECKMARK}  {} compiled to {}",
        style(&name).bold(),
        style(out.display()).cyan(),
    );

    Ok(())
}

/// Color mode for Typst diagnostics.
pub(crate) fn diagnostic_color() -> DiagnosticColor {
    if console::colors_enabled_stderr() {
        DiagnosticColor::Ansi
    } else {
        DiagnosticColor::None
    }
}

pub(crate) fn build_file_name(args: &CompileArgs, template: &Template<NativeTemplate>) -> String {
    args.name
        .replace("{template}", &template.manifest().package.name)
        .replace(
            "{version}",
            &template.manifest().package.version.to_string(),
        )
        .replace("{timestamp}", &Utc::now().timestamp_millis().to_string())
        .replace("{format}", args.format.file_ending())
}

fn split_pair(pair: &str) -> anyhow::Result<(&str, &str)> {
    pair.split_once('=')
        .ok_or_else(|| anyhow!("Invalid key-value pair '{pair}', expected <key>=<path>."))
}

pub(crate) fn build_inputs(args: &CompileArgs) -> anyhow::Result<TemplateInputs> {
    let mut inputs = TemplateInputs::new();
    if args.development {
        inputs.with_config(CompilationConfig::development());
    }
    for pair in &args.json {
        let (key, path) = split_pair(pair)?;
        let input = read_to_string(path).context("Failed to read json input file.")?;
        inputs.with_input(JsonInput::new(key, input));
    }

    let mut blobs = HashMap::new();
    for pair in &args.blob {
        let (key, path) = split_pair(pair)?;
        let blob = read(path).context("Failed to read blob input file.")?;
        blobs.insert(key.to_owned(), BlobInput::new(key, blob));
    }
    for pair in &args.blob_meta {
        let (key, path) = split_pair(pair)?;
        let Some(blob) = blobs.get_mut(key) else {
            bail!("Blob metadata '{pair}' has no blob input with the key '{key}'.");
        };
        let meta = read_to_string(path).context("Failed to read json file as blob metadata.")?;
        blob.value.metadata = serde_json::from_str(&meta)
            .context("Failed to convert blob metadata to a Typst dictionary.")?;
    }

    for (_, blob) in blobs {
        inputs.with_input(blob);
    }

    Ok(inputs)
}
