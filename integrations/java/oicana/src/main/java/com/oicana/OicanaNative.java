package com.oicana;

import java.util.Map;

/**
 * JNI bridge to the native Oicana library.
 * This class is not part of the public API.
 */
class OicanaNative {
    static {
        NativeLoader.load();
    }

    static native String registerTemplate(
            String templateId,
            byte[] files,
            Map<String, String> jsonInputs,
            Map<String, NativeBlobWithMetadata> blobInputs,
            int compilationMode,
            long maxEntries,
            long maxTotalDecompressedBytes
    );

    /**
     * Returns a two-element array of {@code byte[]} document and warnings
     * {@code String} (or {@code null} if there were none).
     * {@code pageRange} may be {@code null} to export the whole document.
     */
    static native Object[] exportTemplateOnce(
            byte[] files,
            Map<String, String> jsonInputs,
            Map<String, NativeBlobWithMetadata> blobInputs,
            int compilationMode,
            String exportFormat,
            String pageRange,
            long maxEntries,
            long maxTotalDecompressedBytes
    );

    static native String compileTemplate(
            String templateId,
            Map<String, String> jsonInputs,
            Map<String, NativeBlobWithMetadata> blobInputs,
            int compilationMode
    );

    /** {@code pageRange} may be {@code null} to export the whole document. */
    static native byte[] exportDocument(String documentId, String exportFormat, String pageRange);

    static native void removeDocument(String documentId);

    /** Returns the compilation warnings for the document, or {@code null} if there were none. */
    static native String getWarnings(String documentId);

    static native void removeWorld(String templateId);

    static native String manifest(String templateId);

    static native String documentPages(String documentId);

    static native String getSource(String templateId, String file);

    static native byte[] getFile(String templateId, String file);

    static native void setValidateInputs(String templateId, boolean validate);

    static native void configureAutomaticCacheEviction(int maxAge);

    static native void configureDiagnosticColor(boolean ansi);

    static native void evictCache(int maxAge);

    static native int registerFont(byte[] font);

    static native int registerFontPath(String path);

    /** Flattened {@code [family, path, ...]}, two entries per face. */
    static native Object[] registeredFonts();

    static native void clearFonts();
}
