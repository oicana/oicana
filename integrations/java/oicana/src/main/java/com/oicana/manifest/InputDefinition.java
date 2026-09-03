package com.oicana.manifest;

/**
 * An input a template declares.
 */
public sealed interface InputDefinition permits JsonInputDefinition, BlobInputDefinition {

    /**
     * Key the input is supplied and used under.
     *
     * @return the input key
     */
    String key();

    /**
     * Whether a value of this input is required for compilation.
     *
     * @return whether a value is required
     */
    boolean required();
}
