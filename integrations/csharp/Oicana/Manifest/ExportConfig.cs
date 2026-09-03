namespace Oicana.Manifest;

/// <summary>
/// How compiled documents are exported.
/// </summary>
public sealed class ExportConfig
{
    /// <summary>
    /// PDF export configuration.
    /// </summary>
    public required PdfExportConfig Pdf { get; init; }
}
