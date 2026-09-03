namespace Oicana.Manifest;

/// <summary>
/// The Oicana configuration of a template.
/// </summary>
public sealed class OicanaConfig
{
    /// <summary>
    /// Version of the manifest format.
    /// </summary>
    public required int ManifestVersion { get; init; }

    /// <summary>
    /// The inputs the template declares, in manifest order.
    /// </summary>
    public required IReadOnlyList<InputDefinition> Inputs { get; init; }

    /// <summary>
    /// Whether JSON inputs are validated against their schemas by default.
    /// </summary>
    public required bool ValidateJsonInputsByDefault { get; init; }

    /// <summary>
    /// How compiled documents are exported.
    /// </summary>
    public required ExportConfig Export { get; init; }

    /// <summary>
    /// Fonts the template expects from its host.
    /// </summary>
    public required FontConfig Fonts { get; init; }
}
