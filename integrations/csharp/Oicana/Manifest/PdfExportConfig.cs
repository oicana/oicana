namespace Oicana.Manifest;

/// <summary>
/// How documents are exported to PDF.
/// </summary>
public sealed class PdfExportConfig
{
    /// <summary>
    /// PDF standards the export conforms to, for example <c>a-3b</c>.
    /// </summary>
    public required IReadOnlyList<string> Standards { get; init; }

    /// <summary>
    /// Whether the PDF is tagged for accessibility.
    /// </summary>
    public required bool Tagged { get; init; }
}
