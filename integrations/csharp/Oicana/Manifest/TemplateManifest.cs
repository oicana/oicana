using System.Text.Json;

namespace Oicana.Manifest;

/// <summary>
/// A template's manifest.
/// </summary>
public sealed class TemplateManifest
{
    /// <summary>
    /// The manifest is camelCase on the wire, the properties here are PascalCase.
    /// </summary>
    private static readonly JsonSerializerOptions Options = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
    };

    /// <summary>
    /// The Typst package section of the manifest.
    /// </summary>
    public required PackageInfo Package { get; init; }

    /// <summary>
    /// The Oicana section of the manifest.
    /// </summary>
    public required OicanaConfig Oicana { get; init; }

    /// <summary>
    /// Parse a manifest from the JSON the native library returns.
    /// </summary>
    /// <param name="json">The serialized manifest.</param>
    /// <exception cref="OicanaException">If the manifest cannot be parsed.</exception>
    /// <returns>The parsed manifest.</returns>
    public static TemplateManifest FromJson(string json)
    {
        try
        {
            return JsonSerializer.Deserialize<TemplateManifest>(json, Options)
                   ?? throw new OicanaException("The template manifest is empty");
        }
        catch (JsonException exception)
        {
            throw new OicanaException($"Failed to parse the template manifest: {exception.Message}");
        }
    }
}
