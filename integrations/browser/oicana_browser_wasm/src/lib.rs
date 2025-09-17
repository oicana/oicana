//! Lower level WASM bindings for Oicana.
//!
//! You most likely want to use the npm package `@oicana/browser` instead.

use js_sys::Uint8Array;
use log::{info, warn, Level};
use oicana_export::pdf::export_merged_pdf;
use oicana_export::png::export_merged_png;
use oicana_export::svg::export_merged_svg;
use oicana_files::packed::PackedTemplate;
use oicana_files::TemplateFiles;
use oicana_input::input::blob::{Blob, BlobInput};
use oicana_input::input::json::JsonInput;
use oicana_input::{CompilationConfig, TemplateInputs};
use oicana_world::diagnostics::DiagnosticColor;
use oicana_world::get_current_time;
use oicana_world::manifest::OicanaWorldFiles;
use oicana_world::world::OicanaWorld;
use once_cell::sync::OnceCell;
use serde::Deserialize;
use serde_wasm_bindgen::from_value;
use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Mutex;
use typst::foundations::Bytes;
use typst::layout::PagedDocument;
use typst::syntax::{FileId, VirtualPath};
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

/// Error string when a requested template is not registered yet. Call `[register_template]` before
/// trying to use the template through a different method.
pub const NOT_REGISTERED: &str = "Template is not registered";

/// Register the given template. This will read the template as a [`PackedTemplate`] and compile it
/// once with the given inputs. The Typst [`typst::World`] will be cached and reused for subsequent
/// calls to the other methods with the same template identifier.
#[wasm_bindgen]
pub fn register_template(
    template: String,
    files: &Uint8Array,
    json_inputs: JsValue,
    blob_inputs: JsValue,
    export_format: JsValue,
    compilation_mode: JsValue,
) -> Result<Uint8Array, String> {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(Level::Debug);
    let start = get_current_time();

    let mut inputs = prepare_inputs(json_inputs, blob_inputs)?;
    let export_format: ExportFormat = from_value(export_format)
        .map_err(|error| format!("Failed to convert to export format: {error:?}"))?;
    let compilation_mode: CompilationMode = from_value(compilation_mode)
        .map_err(|error| format!("Failed to convert to compilation mode: {error:?}"))?;
    inputs.with_config(compilation_mode.into());

    let mut vec = vec![0; files.length() as usize];
    files.copy_to(&mut vec[..]);
    let files = PackedTemplate::new(Cursor::new(vec));
    let manifest = files.manifest().map_err(|error| format!("{error:?}"))?;
    println!("inserting new world for template '{template}'");

    let mut world =
        OicanaWorld::new(files, inputs, manifest).map_err(|error| format!("{error:?}"))?;
    world.color = DiagnosticColor::None;

    let document = world.compile().map_err(|error| format!("{error:?}"))?;
    let document_time = get_current_time();
    info!("Done compiling document in {}ms", document_time - start);

    let result = export(&document.document, &world, export_format);
    world_cache().lock().unwrap().insert(template, world);

    result
}

/// Compile the identified template with the given inputs.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[wasm_bindgen]
pub fn compile_template(
    template: String,
    json_inputs: JsValue,
    blob_inputs: JsValue,
    export_format: JsValue,
    compilation_mode: JsValue,
) -> Result<Uint8Array, String> {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(Level::Debug);
    let start = get_current_time();

    let mut cache_lock = world_cache().lock().unwrap();
    let Some(world) = cache_lock.get_mut(&template) else {
        return Err(NOT_REGISTERED.to_owned());
    };
    let export_format: ExportFormat = from_value(export_format)
        .map_err(|error| format!("Failed to convert to export format: {error:?}"))?;
    let compilation_mode: CompilationMode = from_value(compilation_mode)
        .map_err(|error| format!("Failed to convert to compilation mode: {error:?}"))?;
    let mut inputs = prepare_inputs(json_inputs, blob_inputs)?;
    inputs.with_config(compilation_mode.into());
    world.update_inputs(inputs);

    let document = world.compile().map_err(|error| format!("{error:?}"))?;
    let document_time = get_current_time();
    if let Some(warnings) = document.warnings {
        warn!("{warnings}");
    }
    info!("Done preparing document in {}ms", document_time - start);

    export(&document.document, world, export_format)
}

/// Load all input definitions for the given template.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[wasm_bindgen]
pub fn inputs(template: String) -> Result<String, String> {
    let mut cache_lock = world_cache().lock().unwrap();
    let Some(world) = cache_lock.get_mut(&template) else {
        return Err(NOT_REGISTERED.to_owned());
    };
    let template = world
        .files
        .manifest()
        .map_err(|error| format!("{error:?}"))?
        .tool
        .oicana;

    serde_json::ser::to_string(&template).map_err(|error| format!("{error:?}"))
}

/// Load the source of the given file in the template.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[wasm_bindgen]
pub fn get_source(template: String, file: String) -> Result<String, String> {
    let mut cache_lock = world_cache().lock().unwrap();
    let Some(world) = cache_lock.get_mut(&template) else {
        return Err(NOT_REGISTERED.to_owned());
    };
    world
        .files
        .source(FileId::new(None, VirtualPath::new(file)))
        .map_err(|error| format!("{error}"))
        .map(|source| source.text().to_string())
}

/// Load the source of the given file in the template.
///
/// Calling this method requires a previous call to [`register_template`] with the same template
/// identifier.
#[wasm_bindgen]
pub fn get_file(template: String, file: String) -> Result<Uint8Array, String> {
    let mut cache_lock = world_cache().lock().unwrap();
    let Some(world) = cache_lock.get_mut(&template) else {
        return Err(NOT_REGISTERED.to_owned());
    };
    let bytes = world
        .files
        .file(FileId::new(None, VirtualPath::new(file)))
        .map_err(|error| format!("{error}"))?;
    let array = Uint8Array::new_with_length(bytes.len() as u32);
    array.copy_from(&bytes);
    Ok(array)
}

fn prepare_inputs(json_inputs: JsValue, blobs: JsValue) -> Result<TemplateInputs, String> {
    let start = get_current_time();
    let mut inputs = TemplateInputs::new();
    add_blobs(&mut inputs, blobs)?;
    let blob_time = get_current_time();
    info!("Created blob map in {}ms", blob_time - start);
    add_json_inputs(&mut inputs, json_inputs)?;
    let inputs_time = get_current_time();
    info!("Created json input map in {}ms", inputs_time - blob_time);

    Ok(inputs)
}

fn export(
    document: &PagedDocument,
    world: &OicanaWorld<PackedTemplate>,
    export_format: ExportFormat,
) -> Result<Uint8Array, String> {
    match export_format {
        ExportFormat::Png { pixels_per_pt } => {
            let start_time = get_current_time();
            let pix_map_result = export_merged_png(document, pixels_per_pt);
            info!("Rendered image in {}ms", get_current_time() - start_time);
            pix_map_result
                .map_err(|error| format!("Failed to encode PNG: {error:?}"))
                .map(|pix_map| bytes_to_js_array(&pix_map))
        }
        ExportFormat::Pdf => export_merged_pdf(document, world).map(|pdf| bytes_to_js_array(&pdf)),
        ExportFormat::Svg => {
            let svg = export_merged_svg(document);

            Ok(bytes_to_js_array(&svg))
        }
    }
}

fn bytes_to_js_array(bytes: &[u8]) -> Uint8Array {
    let uint8_array = Uint8Array::new_with_length(bytes.len() as u32);
    uint8_array.copy_from(bytes);

    uint8_array
}

#[derive(Deserialize)]
enum CompilationMode {
    #[serde(alias = "production")]
    Production,
    #[serde(alias = "development")]
    Development,
}

impl From<CompilationMode> for oicana_input::CompilationConfig {
    fn from(value: CompilationMode) -> Self {
        match value {
            CompilationMode::Development => CompilationConfig::development(),
            CompilationMode::Production => CompilationConfig::production(),
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "format")]
enum ExportFormat {
    #[serde(alias = "png")]
    Png { pixels_per_pt: f32 },
    #[serde(alias = "pdf")]
    Pdf,
    #[serde(alias = "svg")]
    Svg,
}

#[derive(Deserialize)]
struct BlobWithMetadata {
    bytes: Vec<u8>,
    meta: serde_json::Value,
}

fn add_blobs(inputs: &mut TemplateInputs, blobs: JsValue) -> Result<(), String> {
    let blobs: HashMap<String, BlobWithMetadata> = from_value(blobs)
        .map_err(|error| format!("Failed to deserialize HashMap<String, BlobWithMetadata> from JavaScript value: {error:?}"))?;
    for (key, value) in blobs {
        let mut blob = Blob::from(Bytes::new(value.bytes.to_vec()));
        blob.metadata = Deserialize::deserialize(value.meta)
            .map_err(|error| format!("Failed to deserialize from JSON value: {error:?}"))?;
        inputs.with_input(BlobInput::new(key, blob));
    }

    Ok(())
}

fn add_json_inputs(inputs: &mut TemplateInputs, json_inputs: JsValue) -> Result<(), String> {
    let json_inputs: HashMap<String, String> = from_value(json_inputs).map_err(|error| {
        format!("Failed to deserialize HashMap<String, String> from JavaScript value: {error:?}")
    })?;
    json_inputs
        .into_iter()
        .map(|(key, value)| JsonInput::new(key, value))
        .for_each(|input| {
            inputs.with_input(input);
        });

    Ok(())
}

fn world_cache() -> &'static Mutex<HashMap<String, OicanaWorld<PackedTemplate>>> {
    static ZIPPED_WORLD: OnceCell<Mutex<HashMap<String, OicanaWorld<PackedTemplate>>>> =
        OnceCell::new();
    ZIPPED_WORLD.get_or_init(|| Mutex::new(HashMap::new()))
}
