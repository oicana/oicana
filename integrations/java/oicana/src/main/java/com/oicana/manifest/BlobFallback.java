package com.oicana.manifest;

import java.util.Map;

/**
 * A blob from the template, used when no value is supplied.
 *
 * @param file File in the template holding the blob
 * @param meta Metadata passed to the template along with the blob, or {@code null}
 */
public record BlobFallback(String file, Map<String, Object> meta) {}
