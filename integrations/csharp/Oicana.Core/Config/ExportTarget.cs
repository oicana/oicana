namespace Oicana.Config;

/// <summary>
/// Formats that an Oicana template can be compiled into.
/// </summary>
public enum ExportTarget
{
    /// <summary>
    /// Export to a PDF file.
    ///
    /// The PDF standards are configured in the template manifest
    /// </summary>
    Pdf = 0,
    /// <summary>
    /// Export to a PNG image.
    /// </summary>
    /// <remarks>The image is not optimized for file size.</remarks>
    Png = 1,
    /// <summary>
    /// Export to an SVG file.
    /// </summary>
    Svg = 2,
}
