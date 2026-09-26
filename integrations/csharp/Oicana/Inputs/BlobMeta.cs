using System.Text.Json.Nodes;

namespace Oicana.Inputs;

/// <summary>
/// Meta data for a blob input set
/// </summary>
public class BlobMeta
{
    /// <summary>
    /// In case the blob is an image, this is its format.
    /// The format is used in Typst to decode the image data.
    /// For possible values, see https://typst.app/docs/reference/visualize/image/#definitions-decode-format
    /// </summary>
    /// <example>png</example>
    public string? ImageFormat { get; set; }

    /// <summary>
    /// Custom meta data
    /// </summary>
    public JsonObject? Custom { get; set; }

    /// <summary>
    /// Build a single JsonObject from the blob meta
    /// </summary>
    public JsonObject Build()
    {
        var meta = Custom?.DeepClone().AsObject() ?? new JsonObject();

        if (ImageFormat != null)
        {
            meta.TryAdd("image_format", ImageFormat);
        }

        return meta;
    }
}
