package com.oicana;

import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Optional;

/**
 * A compiled document kept in memory so it can be exported on demand without re-compiling.
 *
 * Obtain one via {@link Template#compile(java.util.Map, java.util.Map, CompilationMode)}. Use
 * try-with-resources or call {@link #close()} to free the underlying document.
 *
 * <pre>{@code
 * try (var document = template.compile(inputs, blobs)) {
 *     byte[] pdf = document.exportPdf();
 *     byte[] firstPage = document.exportPng(2.0f, PageRange.single(0));
 * }
 * }</pre>
 */
public class CompiledDocument implements AutoCloseable {

    private volatile String documentId;
    private final List<PageSize> pages;
    private final String warnings;

    CompiledDocument(String documentId) {
        this.documentId = documentId;
        this.pages = parsePageSizes(OicanaNative.documentPages(documentId));
        this.warnings = OicanaNative.getWarnings(documentId);
    }

    /**
     * The sizes (in points) of every page, in document order.
     *
     * @return an unmodifiable list of page sizes
     */
    public List<PageSize> pages() {
        return pages;
    }

    /**
     * The number of pages in the document.
     *
     * @return the page count
     */
    public int pageCount() {
        return pages.size();
    }

    /**
     * Warnings produced by the compilation of this document.
     *
     * @return the warnings, or an empty Optional if there were none
     */
    public Optional<String> warnings() {
        return Optional.ofNullable(warnings);
    }

    /**
     * Export the whole document in the given format.
     *
     * @param exportFormat the output format
     * @return the exported document as a byte array
     */
    public byte[] export(ExportFormat exportFormat) {
        return export(exportFormat, null);
    }

    /**
     * Export the document in the given format, optionally restricted to a range of pages.
     *
     * @param exportFormat the output format
     * @param pages the 0-based, inclusive page range to export, or {@code null} for the whole
     *     document
     * @return the exported document as a byte array
     */
    public byte[] export(ExportFormat exportFormat, PageRange pages) {
        ensureOpen();
        return OicanaNative.exportDocument(
                documentId,
                exportFormat.toJsonString(),
                pages == null ? null : pages.toJsonString());
    }

    /**
     * Export the whole document to PDF.
     *
     * @return the PDF document as a byte array
     */
    public byte[] exportPdf() {
        return export(ExportFormat.pdf(), null);
    }

    /**
     * Export the document to PDF, optionally restricted to a range of pages.
     * Tagging will be automatically turned off when exporting a subset of pages.
     *
     * @param pages the 0-based, inclusive page range to export, or {@code null} for the whole
     *     document
     * @return the PDF document as a byte array
     */
    public byte[] exportPdf(PageRange pages) {
        return export(ExportFormat.pdf(), pages);
    }

    /**
     * Export the whole document to PNG at the default resolution of 1 pixel per point.
     * Multiple pages are merged into a single, vertically stacked image.
     *
     * @return the PNG image as a byte array
     */
    public byte[] exportPng() {
        return exportPng(1.0f, null);
    }

    /**
     * Export the whole document to PNG.
     * Multiple pages are merged into a single, vertically stacked image.
     *
     * @param pixelsPerPt resolution in pixels per point
     * @return the PNG image as a byte array
     */
    public byte[] exportPng(float pixelsPerPt) {
        return exportPng(pixelsPerPt, null);
    }

    /**
     * Export the document to PNG at the default resolution of 1 pixel per point, optionally
     * restricted to a range of pages.
     * Multiple pages are merged into a single, vertically stacked image.
     *
     * @param pages the 0-based, inclusive page range to export, or {@code null} for the whole
     *     document
     * @return the PNG image as a byte array
     */
    public byte[] exportPng(PageRange pages) {
        return exportPng(1.0f, pages);
    }

    /**
     * Export the document to PNG, optionally restricted to a range of pages.
     * Multiple pages are merged into a single, vertically stacked image.
     *
     * @param pixelsPerPt resolution in pixels per point
     * @param pages the 0-based, inclusive page range to export, or {@code null} for the whole
     *     document
     * @return the PNG image as a byte array
     */
    public byte[] exportPng(float pixelsPerPt, PageRange pages) {
        return export(ExportFormat.png(pixelsPerPt), pages);
    }

    /**
     * Export the whole document to SVG.
     *
     * @return the SVG document as a byte array
     */
    public byte[] exportSvg() {
        return export(ExportFormat.svg(), null);
    }

    /**
     * Export the document to SVG, optionally restricted to a range of pages.
     *
     * @param pages the 0-based, inclusive page range to export, or {@code null} for the whole
     *     document
     * @return the SVG document as a byte array
     */
    public byte[] exportSvg(PageRange pages) {
        return export(ExportFormat.svg(), pages);
    }

    /** Release the cached document. The instance must not be used after calling close(). */
    @Override
    public void close() {
        if (documentId != null) {
            OicanaNative.removeDocument(documentId);
            documentId = null;
        }
    }

    private void ensureOpen() {
        if (documentId == null) {
            throw new IllegalStateException("CompiledDocument has already been closed");
        }
    }

    private static List<PageSize> parsePageSizes(String json) {
        List<PageSize> sizes = new ArrayList<>();
        try {
            for (JsonElement page : JsonParser.parseString(json).getAsJsonArray()) {
                JsonObject size = page.getAsJsonObject();
                sizes.add(new PageSize(size.get("width").getAsDouble(),
                        size.get("height").getAsDouble()));
            }
        } catch (RuntimeException error) {
            throw new OicanaException("Failed to read the page sizes of the document", error);
        }
        return Collections.unmodifiableList(sizes);
    }
}
