using System.Text.Json.Serialization;

namespace Oicana.Manifest;

/// <summary>
/// An input a template declares.
/// </summary>
[JsonPolymorphic(TypeDiscriminatorPropertyName = "type")]
[JsonDerivedType(typeof(JsonInputDefinition), "json")]
[JsonDerivedType(typeof(BlobInputDefinition), "blob")]
public abstract class InputDefinition
{
    /// <summary>
    /// Key the input is supplied and used under.
    /// </summary>
    public required string Key { get; init; }

    /// <summary>
    /// Whether a value of this input is required for compilation.
    /// </summary>
    public required bool Required { get; init; }
}
