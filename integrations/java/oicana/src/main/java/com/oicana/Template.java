package com.oicana;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.oicana.manifest.TemplateManifest;
import java.util.HashMap;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

/**
 * A template for generating documents.
 *
 * <p>The template ZIP file is loaded during construction and cached for subsequent compilations.
 * Template instances are thread-safe and can be shared across threads.
 *
 * <p>Use try-with-resources for automatic cleanup:
 * <pre>{@code
 * try (var template = new Template(zipBytes)) {
 *     byte[] pdf = template.export(Map.of("name", "{\"value\": \"World\"}"), Map.of());
 * }
 * }</pre>
 */
public class Template implements AutoCloseable {

    private static final Gson METADATA_GSON =
            new GsonBuilder().serializeNulls().disableHtmlEscaping().create();
    private final String templateId;
    private volatile boolean closed = false;
    private volatile String lastWarnings;

    /**
     * Register a template with the given template file.
     * The template is compiled once in development mode to warm up the cache.
     *
     * @param templateFile the packed Oicana template ZIP file
     */
    public Template(byte[] templateFile) {
        this(templateFile, Map.of(), Map.of(), CompilationMode.DEVELOPMENT);
    }

    /**
     * Register a template with the given template file and inputs.
     *
     * @param templateFile the packed Oicana template ZIP file
     * @param jsonInputs   JSON inputs for the initial warm-up compilation
     * @param blobInputs   blob inputs for the initial warm-up compilation
     * @param mode         compilation mode for the initial warm-up compilation
     */
    public Template(byte[] templateFile, Map<String, String> jsonInputs,
                    Map<String, BlobInput> blobInputs, CompilationMode mode) {
        this(templateFile, jsonInputs, blobInputs, mode, null);
    }

    /**
     * Register a template with the given template file, inputs, and zip limits.
     *
     * @param templateFile the packed Oicana template ZIP file
     * @param jsonInputs   JSON inputs for the initial warm-up compilation
     * @param blobInputs   blob inputs for the initial warm-up compilation
     * @param mode         compilation mode for the initial warm-up compilation
     * @param limits       limits for reading the template ZIP, or {@code null} for the defaults
     */
    public Template(byte[] templateFile, Map<String, String> jsonInputs,
                    Map<String, BlobInput> blobInputs, CompilationMode mode, ZipLimits limits) {
        this.templateId = UUID.randomUUID().toString();
        String documentId = OicanaNative.registerTemplate(
                this.templateId,
                templateFile,
                jsonInputs,
                convertBlobInputs(blobInputs),
                mode.value,
                maxEntries(limits),
                maxTotalDecompressedBytes(limits)
        );
        this.lastWarnings = OicanaNative.getWarnings(documentId);
        OicanaNative.removeDocument(documentId);
    }

    /**
     * Warnings produced by the most recent compilation.
     *
     * @return the warnings, or an empty Optional if there were none
     */
    public Optional<String> warnings() {
        return Optional.ofNullable(lastWarnings);
    }

    /**
     * Compile the template to PDF without any inputs in production mode.
     *
     * @return the compiled document as a byte array
     */
    public byte[] export() {
        return export(Map.of(), Map.of(), ExportFormat.pdf(), CompilationMode.PRODUCTION);
    }

    /**
     * Compile the template without any inputs using the given export format and compilation mode.
     *
     * @param exportFormat the output format
     * @param mode         the compilation mode
     * @return the compiled document as a byte array
     */
    public byte[] export(ExportFormat exportFormat, CompilationMode mode) {
        return export(Map.of(), Map.of(), exportFormat, mode);
    }

    /**
     * Compile the template to PDF with the given inputs in production mode.
     *
     * @param jsonInputs the JSON inputs for the template
     * @param blobInputs the blob inputs for the template
     * @return the compiled document as a byte array
     */
    public byte[] export(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs) {
        return export(jsonInputs, blobInputs, ExportFormat.pdf(), CompilationMode.PRODUCTION);
    }

    /**
     * Compile the template with the given inputs, export format, and compilation mode.
     *
     * @param jsonInputs   the JSON inputs for the template
     * @param blobInputs   the blob inputs for the template
     * @param exportFormat the output format
     * @param mode         the compilation mode
     * @return the compiled document as a byte array
     */
    public byte[] export(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs,
                          ExportFormat exportFormat, CompilationMode mode) {
        return export(jsonInputs, blobInputs, exportFormat, mode, null);
    }

    /**
     * Compile the template and export a range of pages.
     *
     * @param jsonInputs   the JSON inputs for the template
     * @param blobInputs   the blob inputs for the template
     * @param exportFormat the output format
     * @param mode         the compilation mode
     * @param pages        the 0-based, inclusive page range to export, or {@code null} for the
     *                     whole document
     * @return the compiled document as a byte array
     */
    public byte[] export(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs,
                          ExportFormat exportFormat, CompilationMode mode, PageRange pages) {
        ensureNotClosed();
        String documentId = OicanaNative.compileTemplate(
                this.templateId,
                jsonInputs,
                convertBlobInputs(blobInputs),
                mode.value
        );
        this.lastWarnings = OicanaNative.getWarnings(documentId);
        try {
            return OicanaNative.exportDocument(
                    documentId,
                    exportFormat.toJsonString(),
                    pages == null ? null : pages.toJsonString()
            );
        } finally {
            OicanaNative.removeDocument(documentId);
        }
    }

    /**
     * Compile the template to PDF without any inputs in production mode.
     *
     * @return the PDF document as a byte array
     */
    public byte[] exportPdf() {
        return exportPdf(Map.of(), Map.of());
    }

    /**
     * Compile the template to PDF with the given inputs in production mode.
     *
     * @param jsonInputs the JSON inputs for the template
     * @param blobInputs the blob inputs for the template
     * @return the PDF document as a byte array
     */
    public byte[] exportPdf(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs) {
        return exportPdf(jsonInputs, blobInputs, CompilationMode.PRODUCTION, null);
    }

    /**
     * Compile the template and export it to PDF, optionally restricted to a range of pages.
     * Tagging will be automatically turned off when exporting a subset of pages.
     *
     * @param jsonInputs the JSON inputs for the template
     * @param blobInputs the blob inputs for the template
     * @param mode       the compilation mode
     * @param pages      the 0-based, inclusive page range to export, or {@code null} for the
     *                   whole document
     * @return the PDF document as a byte array
     */
    public byte[] exportPdf(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs,
                             CompilationMode mode, PageRange pages) {
        return export(jsonInputs, blobInputs, ExportFormat.pdf(), mode, pages);
    }

    /**
     * Compile the template to PNG without any inputs in production mode, at the default
     * resolution of 1 pixel per point.
     * Multiple pages are merged into a single, vertically stacked image.
     *
     * @return the PNG image as a byte array
     */
    public byte[] exportPng() {
        return exportPng(Map.of(), Map.of(), 1.0f);
    }

    /**
     * Compile the template to PNG without any inputs in production mode.
     * Multiple pages are merged into a single, vertically stacked image.
     *
     * @param pixelsPerPt resolution in pixels per point
     * @return the PNG image as a byte array
     */
    public byte[] exportPng(float pixelsPerPt) {
        return exportPng(Map.of(), Map.of(), pixelsPerPt);
    }

    /**
     * Compile the template to PNG with the given inputs in production mode, at the default
     * resolution of 1 pixel per point.
     * Multiple pages are merged into a single, vertically stacked image.
     *
     * @param jsonInputs the JSON inputs for the template
     * @param blobInputs the blob inputs for the template
     * @return the PNG image as a byte array
     */
    public byte[] exportPng(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs) {
        return exportPng(jsonInputs, blobInputs, 1.0f);
    }

    /**
     * Compile the template to PNG with the given inputs in production mode.
     * Multiple pages are merged into a single, vertically stacked image.
     *
     * @param jsonInputs  the JSON inputs for the template
     * @param blobInputs  the blob inputs for the template
     * @param pixelsPerPt resolution in pixels per point
     * @return the PNG image as a byte array
     */
    public byte[] exportPng(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs,
                             float pixelsPerPt) {
        return exportPng(jsonInputs, blobInputs, CompilationMode.PRODUCTION, pixelsPerPt, null);
    }

    /**
     * Compile the template and export it to PNG, optionally restricted to a range of pages.
     * Multiple pages are merged into a single, vertically stacked image.
     *
     * @param jsonInputs  the JSON inputs for the template
     * @param blobInputs  the blob inputs for the template
     * @param mode        the compilation mode
     * @param pixelsPerPt resolution in pixels per point
     * @param pages       the 0-based, inclusive page range to export, or {@code null} for the
     *                    whole document
     * @return the PNG image as a byte array
     */
    public byte[] exportPng(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs,
                             CompilationMode mode, float pixelsPerPt, PageRange pages) {
        return export(jsonInputs, blobInputs, ExportFormat.png(pixelsPerPt), mode, pages);
    }

    /**
     * Compile the template to SVG without any inputs in production mode.
     *
     * @return the SVG document as a byte array
     */
    public byte[] exportSvg() {
        return exportSvg(Map.of(), Map.of());
    }

    /**
     * Compile the template to SVG with the given inputs in production mode.
     *
     * @param jsonInputs the JSON inputs for the template
     * @param blobInputs the blob inputs for the template
     * @return the SVG document as a byte array
     */
    public byte[] exportSvg(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs) {
        return exportSvg(jsonInputs, blobInputs, CompilationMode.PRODUCTION, null);
    }

    /**
     * Compile the template and export it to SVG, optionally restricted to a range of pages.
     *
     * @param jsonInputs the JSON inputs for the template
     * @param blobInputs the blob inputs for the template
     * @param mode       the compilation mode
     * @param pages      the 0-based, inclusive page range to export, or {@code null} for the
     *                   whole document
     * @return the SVG document as a byte array
     */
    public byte[] exportSvg(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs,
                             CompilationMode mode, PageRange pages) {
        return export(jsonInputs, blobInputs, ExportFormat.svg(), mode, pages);
    }

    /**
     * Compile the template in production mode without any inputs.
     *
     * @return a handle to the compiled document
     */
    public CompiledDocument compile() {
        return compile(Map.of(), Map.of(), CompilationMode.PRODUCTION);
    }

    /**
     * Compile the template with the given inputs in production mode.
     *
     * @param jsonInputs the JSON inputs for the template
     * @param blobInputs the blob inputs for the template
     * @return a handle to the compiled document
     */
    public CompiledDocument compile(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs) {
        return compile(jsonInputs, blobInputs, CompilationMode.PRODUCTION);
    }

    /**
     * Compile the template.
     *
     * <p>Unlike {@link #export(Map, Map, ExportFormat, CompilationMode)}, the document is kept in
     * memory so it can be exported one or more times without re-compiling. Use try-with-resources
     * or call {@link CompiledDocument#close()} to free it.
     *
     * @param jsonInputs the JSON inputs for the template
     * @param blobInputs the blob inputs for the template
     * @param mode       the compilation mode
     * @return a handle to the compiled document
     */
    public CompiledDocument compile(Map<String, String> jsonInputs, Map<String, BlobInput> blobInputs,
                                    CompilationMode mode) {
        ensureNotClosed();
        String documentId = OicanaNative.compileTemplate(
                this.templateId,
                jsonInputs,
                convertBlobInputs(blobInputs),
                mode.value
        );
        CompiledDocument document = new CompiledDocument(documentId);
        this.lastWarnings = document.warnings().orElse(null);
        return document;
    }

    /**
     * Compile and export a template in a single call without caching.
     * Useful for one-off compilations where template reuse is not needed.
     *
     * @param templateFile the packed Oicana template ZIP file
     * @param jsonInputs   the JSON inputs
     * @param blobInputs   the blob inputs
     * @param exportFormat the output format
     * @param mode         the compilation mode
     * @return the exported document and any compilation warnings
     */
    public static ExportOnceResult exportOnce(byte[] templateFile, Map<String, String> jsonInputs,
                                              Map<String, BlobInput> blobInputs,
                                              ExportFormat exportFormat, CompilationMode mode) {
        return exportOnce(templateFile, jsonInputs, blobInputs, exportFormat, mode, null, null);
    }

    /**
     * Compile and export a template in a single call without caching.
     * Useful for one-off compilations where template reuse is not needed.
     *
     * @param templateFile the packed Oicana template ZIP file
     * @param jsonInputs   the JSON inputs
     * @param blobInputs   the blob inputs
     * @param exportFormat the output format
     * @param mode         the compilation mode
     * @param pages        the 0-based, inclusive page range to export, or {@code null} for the
     *                     whole document
     * @param limits       limits for reading the template ZIP, or {@code null} for the defaults
     * @return the exported document and any compilation warnings
     */
    public static ExportOnceResult exportOnce(byte[] templateFile, Map<String, String> jsonInputs,
                                              Map<String, BlobInput> blobInputs,
                                              ExportFormat exportFormat, CompilationMode mode,
                                              PageRange pages, ZipLimits limits) {
        Object[] result = OicanaNative.exportTemplateOnce(
                templateFile,
                jsonInputs,
                convertBlobInputs(blobInputs),
                mode.value,
                exportFormat.toJsonString(),
                pages == null ? null : pages.toJsonString(),
                maxEntries(limits),
                maxTotalDecompressedBytes(limits)
        );
        return new ExportOnceResult((byte[]) result[0], (String) result[1]);
    }

    /**
     * Get the manifest of this template.
     *
     * @return the Typst package section and the Oicana configuration of the template, including
     *     its input definitions
     */
    public TemplateManifest manifest() {
        ensureNotClosed();
        return TemplateManifest.fromJson(OicanaNative.manifest(this.templateId));
    }

    /**
     * Get the source code of a file within the template.
     *
     * @param path the file path within the template
     * @return the source code as a string
     */
    public String source(String path) {
        ensureNotClosed();
        return OicanaNative.getSource(this.templateId, path);
    }

    /**
     * Get the binary content of a file within the template.
     *
     * @param path the file path within the template
     * @return the file content as a byte array
     */
    public byte[] file(String path) {
        ensureNotClosed();
        return OicanaNative.getFile(this.templateId, path);
    }

    /**
     * Enable or disable JSON schema validation for this template.
     *
     * <p>When enabled (the default), JSON inputs are validated against their schemas
     * before compilation.
     *
     * @param validate whether to validate inputs against their JSON schemas
     */
    public void setValidateInputs(boolean validate) {
        ensureNotClosed();
        OicanaNative.setValidateInputs(this.templateId, validate);
    }

    /**
     * Configure automatic cache eviction after each compilation.
     *
     * @param maxAge maximum age threshold. Use -1 to disable eviction, 0 to clear all,
     *               or a positive value to keep entries used within the last n evictions.
     *               Default is 10.
     */
    public static void configureAutomaticCacheEviction(int maxAge) {
        OicanaNative.configureAutomaticCacheEviction(maxAge);
    }

    /**
     * Manually evict the cache with the given age threshold.
     *
     * @param maxAge the age threshold for cache eviction
     */
    public static void evictCache(int maxAge) {
        OicanaNative.evictCache(maxAge);
    }

    /**
     * Release native resources associated with this template.
     * After calling close(), this template instance should not be used.
     */
    @Override
    public void close() {
        if (!closed) {
            closed = true;
            OicanaNative.removeWorld(this.templateId);
        }
    }

    private void ensureNotClosed() {
        if (closed) {
            throw new OicanaException("Template has been closed");
        }
    }

    private static long maxEntries(ZipLimits limits) {
        return limits == null || limits.maxEntries() == null ? -1 : limits.maxEntries();
    }

    private static long maxTotalDecompressedBytes(ZipLimits limits) {
        return limits == null || limits.maxTotalDecompressedBytes() == null
                ? -1
                : limits.maxTotalDecompressedBytes();
    }

    private static Map<String, NativeBlobWithMetadata> convertBlobInputs(Map<String, BlobInput> blobInputs) {
        if (blobInputs == null || blobInputs.isEmpty()) {
            return Map.of();
        }
        Map<String, NativeBlobWithMetadata> result = new HashMap<>(blobInputs.size());
        for (var entry : blobInputs.entrySet()) {
            BlobInput blob = entry.getValue();
            String meta = blob.metadata() == null ? "{}" : metadataToJson(blob.metadata());
            result.put(entry.getKey(), new NativeBlobWithMetadata(blob.data(), meta));
        }
        return result;
    }

    /**
     * Serialize blob metadata for the native layer.
     *
     * {@code null} values are kept, so a template sees {@code none} rather than a missing key.
     */
    static String metadataToJson(Map<String, Object> metadata) {
        return METADATA_GSON.toJson(metadata);
    }
}
