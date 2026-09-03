package com.oicana.manifest;

import java.util.List;

/**
 * How documents are exported to PDF.
 *
 * @param standards PDF standards the export conforms to, for example {@code a-3b}
 * @param tagged Whether the PDF is tagged for accessibility
 */
public record PdfExportConfig(List<String> standards, boolean tagged) {}
