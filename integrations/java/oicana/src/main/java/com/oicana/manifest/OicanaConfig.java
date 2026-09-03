package com.oicana.manifest;

import java.util.List;

/**
 * The Oicana configuration of a template.
 *
 * @param manifestVersion Version of the manifest format
 * @param inputs The inputs the template declares, in manifest order
 * @param validateJsonInputsByDefault Whether JSON inputs are validated against their schemas by
 *     default
 * @param export How compiled documents are exported
 * @param fonts Fonts the template expects from its host
 */
public record OicanaConfig(
        int manifestVersion,
        List<InputDefinition> inputs,
        boolean validateJsonInputsByDefault,
        ExportConfig export,
        FontConfig fonts) {}
