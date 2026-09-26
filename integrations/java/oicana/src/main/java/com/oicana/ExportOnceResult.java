package com.oicana;

import java.util.Optional;

/**
 * Result of a one-shot template export.
 *
 * @param document the exported document
 * @param warnings compilation warnings, or an empty Optional if there were none
 */
public record ExportOnceResult(byte[] document, Optional<String> warnings) {
}
