package com.oicana.manifest;

import java.util.List;

/**
 * Fonts a template expects from its host.
 *
 * @param require Font families the host has to register
 */
public record FontConfig(List<String> require) {}
