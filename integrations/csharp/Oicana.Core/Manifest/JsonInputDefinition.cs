namespace Oicana.Manifest;

/// <summary>
/// An input taking a JSON value.
/// </summary>
public sealed class JsonInputDefinition : InputDefinition
{
    /// <summary>
    /// File in the template holding the value used when none is supplied.
    /// In development mode, <see cref="Development"/> takes precedence.
    /// </summary>
    public string? Default { get; init; }

    /// <summary>
    /// File in the template holding the value used in development mode when none is supplied.
    /// </summary>
    public string? Development { get; init; }

    /// <summary>
    /// File in the template holding the JSON schema of this input.
    /// </summary>
    public string? Schema { get; init; }

    /// <summary>
    /// Whether values are validated against the schema.
    /// </summary>
    public required bool Validate { get; init; }
}
