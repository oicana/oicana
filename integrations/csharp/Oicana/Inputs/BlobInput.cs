using System.Diagnostics.CodeAnalysis;
using System.Text.Json.Nodes;

namespace Oicana.Inputs;

/// <summary>
/// A blob input value for template compilation.
/// Use with a dictionary where the key is the input name.
/// </summary>
public class BlobInput
{
    /// <summary>
    /// Construct a new blob input from bytes
    /// </summary>
    [SetsRequiredMembers]
    public BlobInput(byte[] data) : this(data, null) { }

    /// <summary>
    /// Construct a new blob input from bytes with metadata
    /// </summary>
    [SetsRequiredMembers]
    public BlobInput(byte[] data, BlobMeta? metadata)
    {
        Data = data;
        Metadata = metadata?.Build();
    }

    /// <summary>
    /// Binary data
    /// </summary>
    public required byte[] Data { get; init; }

    /// <summary>
    /// Metadata for the blob (e.g., image format)
    /// </summary>
    public JsonNode? Metadata { get; init; }
}
