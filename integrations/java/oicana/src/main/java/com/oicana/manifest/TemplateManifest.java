package com.oicana.manifest;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonDeserializer;
import com.google.gson.ToNumberPolicy;
import com.google.gson.annotations.SerializedName;

/**
 * A template's manifest.
 *
 * @param packageInfo The Typst package section of the manifest
 * @param oicana The Oicana section of the manifest
 */
public record TemplateManifest(
        @SerializedName("package") PackageInfo packageInfo, OicanaConfig oicana) {

    private static final Gson GSON =
            new GsonBuilder()
                    .setObjectToNumberStrategy(ToNumberPolicy.LONG_OR_DOUBLE)
                    .registerTypeAdapter(
                            InputDefinition.class,
                            (JsonDeserializer<InputDefinition>)
                                    (element, type, context) ->
                                            "json"
                                                            .equals(
                                                                    element.getAsJsonObject()
                                                                            .get("type")
                                                                            .getAsString())
                                                    ? context.deserialize(
                                                            element, JsonInputDefinition.class)
                                                    : context.deserialize(
                                                            element, BlobInputDefinition.class))
                    .create();

    /**
     * Parse a manifest from the JSON the native library returns.
     *
     * @param json the serialized manifest
     * @return the parsed manifest
     */
    public static TemplateManifest fromJson(String json) {
        return GSON.fromJson(json, TemplateManifest.class);
    }
}
