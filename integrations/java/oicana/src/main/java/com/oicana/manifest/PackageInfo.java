package com.oicana.manifest;

import java.util.List;

/**
 * The Typst package a template is.
 *
 * @param name Name of the template
 * @param version Version of the template
 * @param entrypoint File the compilation starts at
 * @param authors Authors of the template
 * @param license License of the template, or {@code null}
 * @param description Short description of the template, or {@code null}
 * @param homepage Web presence of the template, or {@code null}
 * @param repository Repository the template is developed in, or {@code null}
 */
public record PackageInfo(
        String name,
        String version,
        String entrypoint,
        List<String> authors,
        String license,
        String description,
        String homepage,
        String repository) {}
