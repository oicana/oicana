//! The Java (JNI) integration of Oicana.
//!
//! You will want to use this through the `com.oicana:oicana` Maven package.

#![deny(clippy::all)]

use std::collections::HashMap;

use jni::errors::Error;
use jni::objects::{JByteArray, JClass, JMap, JObject, JObjectArray, JString};
use jni::strings::JNIString;
use jni::sys::{jint, jlong};
use jni::{jni_sig, Env, EnvUnowned};

/// Throw a Java OicanaException, returning Error::JavaException for use with `?`.
fn throw_oicana(env: &mut Env, message: &str) -> Error {
    let _ = env.throw_new(
        JNIString::from("com/oicana/OicanaException"),
        JNIString::from(message),
    );
    Error::JavaException
}

fn throw_ffi(env: &mut Env, error: oicana_ffi_core::FfiError) -> Error {
    throw_oicana(env, &error.to_string())
}

/// Safely cast a JObject to JString (the caller must ensure the object is a java.lang.String).
unsafe fn jobject_as_jstring<'local>(env: &Env<'local>, obj: JObject<'local>) -> JString<'local> {
    unsafe { JString::from_raw(env, obj.into_raw()) }
}

/// Extract a HashMap<String, String> from a Java Map<String, String>.
fn extract_string_map<'local>(
    env: &mut Env<'local>,
    map: JObject<'local>,
) -> Result<HashMap<String, String>, Error> {
    if map.is_null() {
        return Ok(HashMap::new());
    }
    let jmap: JMap = env.cast_local::<JMap>(map)?;
    let mut iter = jmap.iter(env)?;
    let mut result = HashMap::new();
    while let Some(entry) = iter.next(env)? {
        let key = entry.key(env)?;
        let value = entry.value(env)?;
        // Safety: we know this is a Map<String, String> from the Java side
        let key_jstr = unsafe { jobject_as_jstring(env, key) };
        let value_jstr = unsafe { jobject_as_jstring(env, value) };
        let key_str = key_jstr.try_to_string(env)?;
        let value_str = value_jstr.try_to_string(env)?;
        result.insert(key_str, value_str);
    }
    Ok(result)
}

/// Extract a HashMap<String, oicana_ffi_core::BlobWithMetadata> from a Java
/// Map<String, BlobWithMetadata>.
fn extract_blob_map<'local>(
    env: &mut Env<'local>,
    map: JObject<'local>,
) -> Result<HashMap<String, oicana_ffi_core::BlobWithMetadata>, Error> {
    if map.is_null() {
        return Ok(HashMap::new());
    }
    let jmap: JMap = env.cast_local::<JMap>(map)?;
    let mut iter = jmap.iter(env)?;
    let mut result = HashMap::new();
    while let Some(entry) = iter.next(env)? {
        let key = entry.key(env)?;
        let value = entry.value(env)?;
        // Safety: keys are Strings from the Java side
        let key_str = unsafe { jobject_as_jstring(env, key) }.try_to_string(env)?;

        // Get bytes field
        let bytes_obj = env
            .get_field(&value, JNIString::from("bytes"), jni_sig!("[B"))?
            .l()?;
        // Safety: we know the field type is byte[]
        let bytes_array = unsafe { JByteArray::from_raw(env, bytes_obj.into_raw()) };
        let bytes = env.convert_byte_array(bytes_array)?;

        // Get meta field
        let meta_obj = env
            .get_field(
                &value,
                JNIString::from("meta"),
                jni_sig!("Ljava/lang/String;"),
            )?
            .l()?;
        // Safety: we know the field type is String
        let meta = unsafe { jobject_as_jstring(env, meta_obj) }.try_to_string(env)?;

        result.insert(key_str, oicana_ffi_core::BlobWithMetadata { bytes, meta });
    }
    Ok(result)
}

fn compilation_mode_from_jint(mode: jint) -> oicana_ffi_core::CompilationMode {
    match mode {
        1 => oicana_ffi_core::CompilationMode::Development,
        _ => oicana_ffi_core::CompilationMode::Production,
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_registerTemplate<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    template_id: JString<'local>,
    files: JByteArray<'local>,
    json_inputs: JObject<'local>,
    blob_inputs: JObject<'local>,
    compilation_mode: jint,
    max_entries: jlong,
    max_total_decompressed_bytes: jlong,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'_>> {
            let template_id = template_id.try_to_string(env)?;
            let file_bytes = env.convert_byte_array(files)?;
            let json_map = extract_string_map(env, json_inputs)?;
            let blob_map = extract_blob_map(env, blob_inputs)?;

            let result_id = oicana_ffi_core::register_template(
                &template_id,
                &file_bytes,
                json_map,
                blob_map,
                compilation_mode_from_jint(compilation_mode),
                oicana_ffi_core::ZipLimits::from_sentinels(
                    max_entries,
                    max_total_decompressed_bytes,
                ),
            )
            .map_err(|e| throw_ffi(env, e))?;

            JString::from_str(env, &result_id)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Compile and export a template once without caching it. Returns a two-element
/// `Object[]` of the document and compilation warnings or null.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_exportTemplateOnce<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    files: JByteArray<'local>,
    json_inputs: JObject<'local>,
    blob_inputs: JObject<'local>,
    compilation_mode: jint,
    export_format: JString<'local>,
    page_range: JString<'local>,
    max_entries: jlong,
    max_total_decompressed_bytes: jlong,
) -> JObjectArray<'local, JObject<'local>> {
    unowned_env
        .with_env(
            |env| -> jni::errors::Result<JObjectArray<'_, JObject<'_>>> {
                let file_bytes = env.convert_byte_array(files)?;
                let json_map = extract_string_map(env, json_inputs)?;
                let blob_map = extract_blob_map(env, blob_inputs)?;
                let export_format_str = export_format.try_to_string(env)?;
                let page_range_str = if page_range.is_null() {
                    None
                } else {
                    Some(page_range.try_to_string(env)?)
                };

                let format = oicana_ffi_core::parse_export_format(&export_format_str)
                    .map_err(|e| throw_ffi(env, e))?;
                let page = oicana_ffi_core::parse_page_range(page_range_str.as_deref())
                    .map_err(|e| throw_ffi(env, e))?;

                let result = oicana_ffi_core::export_once(
                    &file_bytes,
                    json_map,
                    blob_map,
                    compilation_mode_from_jint(compilation_mode),
                    format,
                    page,
                    oicana_ffi_core::ZipLimits::from_sentinels(
                        max_entries,
                        max_total_decompressed_bytes,
                    ),
                )
                .map_err(|e| throw_ffi(env, e))?;

                let array = JObjectArray::<JObject>::new(env, 2, &JObject::null())?;
                let document = env.byte_array_from_slice(&result.bytes)?;
                array.set_element(env, 0, &document)?;
                if let Some(warnings) = result.warnings {
                    let warnings = JString::from_str(env, &warnings)?;
                    array.set_element(env, 1, &warnings)?;
                }
                Ok(array)
            },
        )
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_configureDiagnosticColor<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    ansi: u8,
) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            let color = if ansi != 0 {
                oicana_ffi_core::DiagnosticColor::Ansi
            } else {
                oicana_ffi_core::DiagnosticColor::None
            };
            oicana_ffi_core::configure_diagnostic_color(color);
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Register a single font from its raw file content, returning the number of
/// font faces that were added.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_registerFont<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    font: JByteArray<'local>,
) -> jint {
    unowned_env
        .with_env(|env| -> jni::errors::Result<jint> {
            let data = env.convert_byte_array(font)?;
            Ok(oicana_ffi_core::register_font(data) as jint)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Register a single font file by path, returning the number of font faces that
/// were added. The font data is not retained until it is used.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_registerFontPath<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    path: JString<'local>,
) -> jint {
    unowned_env
        .with_env(|env| -> jni::errors::Result<jint> {
            let path = path.try_to_string(env)?;
            let faces = oicana_ffi_core::register_font_paths(vec![path.into()]);
            Ok(faces as jint)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// All font faces currently registered, flattened into `[family, path, ...]`.
///
/// Two entries per face, with a null path for fonts registered from memory. A
/// flat array rather than JSON: the Java side has no JSON parser, and font
/// families and file paths are arbitrary strings that a regex parser would
/// mangle.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_registeredFonts<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JObjectArray<'local, JObject<'local>> {
    unowned_env
        .with_env(
            |env| -> jni::errors::Result<JObjectArray<'_, JObject<'_>>> {
                let fonts = oicana_ffi_core::registered_fonts();
                let array = JObjectArray::<JObject>::new(env, fonts.len() * 2, &JObject::null())?;
                for (index, font) in fonts.iter().enumerate() {
                    let family = JString::from_str(env, &font.family)?;
                    array.set_element(env, index * 2, &family)?;
                    if let Some(path) = &font.path {
                        let path = JString::from_str(env, path)?;
                        array.set_element(env, index * 2 + 1, &path)?;
                    }
                }
                Ok(array)
            },
        )
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Drop all fonts registered by the host.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_clearFonts<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            oicana_ffi_core::clear_fonts();
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_compileTemplate<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    template_id: JString<'local>,
    json_inputs: JObject<'local>,
    blob_inputs: JObject<'local>,
    compilation_mode: jint,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'_>> {
            let template_id = template_id.try_to_string(env)?;
            let json_map = extract_string_map(env, json_inputs)?;
            let blob_map = extract_blob_map(env, blob_inputs)?;

            let result_id = oicana_ffi_core::compile_template(
                &template_id,
                json_map,
                blob_map,
                compilation_mode_from_jint(compilation_mode),
            )
            .map_err(|e| throw_ffi(env, e))?;

            JString::from_str(env, &result_id)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_exportDocument<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    document_id: JString<'local>,
    export_format: JString<'local>,
    page_range: JString<'local>,
) -> JByteArray<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JByteArray<'_>> {
            let document_id = document_id.try_to_string(env)?;
            let export_format_str = export_format.try_to_string(env)?;
            // A null page range selects the whole document.
            let page_range_str = if page_range.is_null() {
                None
            } else {
                Some(page_range.try_to_string(env)?)
            };

            let format = oicana_ffi_core::parse_export_format(&export_format_str)
                .map_err(|e| throw_ffi(env, e))?;
            let page = oicana_ffi_core::parse_page_range(page_range_str.as_deref())
                .map_err(|e| throw_ffi(env, e))?;
            let bytes = oicana_ffi_core::export_document(&document_id, format, page)
                .map_err(|e| throw_ffi(env, e))?;

            env.byte_array_from_slice(&bytes)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_removeDocument<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    document_id: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let id = document_id.try_to_string(env)?;
            oicana_ffi_core::remove_document(&id);
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Return any compilation warnings produced for the given document, or `null`
/// if there were none. Warnings are cleared when the document is removed.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_getWarnings<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    document_id: JString<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'_>> {
            let id = document_id.try_to_string(env)?;
            match oicana_ffi_core::get_warnings(&id) {
                Some(warnings) => JString::from_str(env, &warnings),
                None => Ok(JString::default()),
            }
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_removeWorld<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    template_id: JString<'local>,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let id = template_id.try_to_string(env)?;
            oicana_ffi_core::remove_world(&id);
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_manifest<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    template_id: JString<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'_>> {
            let template_id = template_id.try_to_string(env)?;
            let json = oicana_ffi_core::manifest(&template_id).map_err(|e| throw_ffi(env, e))?;
            JString::from_str(env, &json)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_documentPages<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    document_id: JString<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'_>> {
            let document_id = document_id.try_to_string(env)?;
            let json =
                oicana_ffi_core::document_pages(&document_id).map_err(|e| throw_ffi(env, e))?;
            JString::from_str(env, &json)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_getSource<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    template_id: JString<'local>,
    file: JString<'local>,
) -> JString<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JString<'_>> {
            let template_id = template_id.try_to_string(env)?;
            let file_path = file.try_to_string(env)?;
            let source = oicana_ffi_core::get_source(&template_id, &file_path)
                .map_err(|e| throw_ffi(env, e))?;
            JString::from_str(env, &source)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_getFile<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    template_id: JString<'local>,
    file: JString<'local>,
) -> JByteArray<'local> {
    unowned_env
        .with_env(|env| -> jni::errors::Result<JByteArray<'_>> {
            let template_id = template_id.try_to_string(env)?;
            let file_path = file.try_to_string(env)?;
            let bytes = oicana_ffi_core::get_file(&template_id, &file_path)
                .map_err(|e| throw_ffi(env, e))?;
            env.byte_array_from_slice(&bytes)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_setValidateInputs<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    template_id: JString<'local>,
    validate: u8,
) {
    unowned_env
        .with_env(|env| -> jni::errors::Result<()> {
            let template_id = template_id.try_to_string(env)?;
            oicana_ffi_core::set_validate_inputs(&template_id, validate != 0)
                .map_err(|e| throw_ffi(env, e))?;
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_configureAutomaticCacheEviction<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    max_age: jint,
) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            let max_age = if max_age < 0 {
                None
            } else {
                Some(max_age as usize)
            };
            oicana_ffi_core::configure_automatic_cache_eviction(max_age);
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_oicana_OicanaNative_evictCache<'local>(
    mut unowned_env: EnvUnowned<'local>,
    _class: JClass<'local>,
    max_age: jint,
) {
    unowned_env
        .with_env(|_env| -> jni::errors::Result<()> {
            if max_age >= 0 {
                oicana_ffi_core::evict_cache(max_age as usize);
            }
            Ok(())
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}
