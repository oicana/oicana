package com.oicana.manifest;

import com.google.gson.annotations.SerializedName;

/**
 * An input taking a JSON value.
 *
 * @param key Key the input is supplied and used under
 * @param required Whether a value of this input is required for compilation
 * @param defaultValue File in the template holding the value used when none is supplied, or
 *     {@code null}. In development mode, {@code development} takes precedence.
 * @param development File in the template holding the value used in development mode when
 *     none is supplied, or {@code null}
 * @param schema File in the template holding the JSON schema of this input, or {@code null}
 * @param validate Whether values are validated against the schema
 */
public record JsonInputDefinition(
        String key,
        boolean required,
        @SerializedName("default") String defaultValue,
        String development,
        String schema,
        boolean validate)
        implements InputDefinition {}
