namespace Oicana.Manifest;

/// <summary>
/// An input taking arbitrary bytes.
/// </summary>
public sealed class BlobInputDefinition : InputDefinition
{
    /// <summary>
    /// Blob used when no value is supplied.
    /// In development mode, <see cref="Development"/> takes precedence.
    /// </summary>
    public BlobFallback? Default { get; init; }

    /// <summary>
    /// Blob used in development mode when no value is supplied.
    /// </summary>
    public BlobFallback? Development { get; init; }
}
