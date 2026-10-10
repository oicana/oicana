namespace Oicana.Manifest;

/// <summary>
/// The Typst package a template is.
/// </summary>
public sealed class PackageInfo
{
    /// <summary>
    /// Name of the template.
    /// </summary>
    public required string Name { get; init; }

    /// <summary>
    /// Version of the template.
    /// </summary>
    public required string Version { get; init; }

    /// <summary>
    /// File the compilation starts at.
    /// </summary>
    public required string Entrypoint { get; init; }

    /// <summary>
    /// Authors of the template.
    /// </summary>
    public required IReadOnlyList<string> Authors { get; init; }

    /// <summary>
    /// License of the template.
    /// </summary>
    public string? License { get; init; }

    /// <summary>
    /// Short description of the template.
    /// </summary>
    public string? Description { get; init; }

    /// <summary>
    /// Web presence of the template.
    /// </summary>
    public string? Homepage { get; init; }

    /// <summary>
    /// Repository the template is developed in.
    /// </summary>
    public string? Repository { get; init; }
}
