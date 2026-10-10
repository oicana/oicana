using System.Text.Json.Nodes;
using Oicana.Interop;
using ExportFormat = Oicana.Config.ExportFormat;
using PageRange = Oicana.Config.PageRange;

namespace Oicana;

/// <summary>
/// A compiled document kept in memory so it can be exported on demand without re-compiling.
///
/// Obtain one via <see cref="Template.Compile"/>. Call <see cref="Dispose"/> (or use a
/// <c>using</c> statement) to free the underlying document.
/// </summary>
public sealed class CompiledDocument : IDisposable
{
    private string? _documentId;

    /// <summary>
    /// Sizes (in points) of every page, in document order.
    /// </summary>
    public IReadOnlyList<PageSize> Pages { get; }

    /// <summary>
    /// Warnings produced by the compilation of this document, or <c>null</c> if there were none.
    /// </summary>
    public string? Warnings { get; }

    /// <summary>
    /// Construct a handle around an already-compiled document. Use <see cref="Template.Compile"/>.
    /// </summary>
    /// <param name="documentId">Identifier of the compiled document in the internal cache.</param>
    internal CompiledDocument(string documentId)
    {
        _documentId = documentId;
        Pages = ParsePageSizes(OicanaFfi.DocumentPages(documentId));
        Warnings = OicanaFfi.GetWarnings(documentId);
    }

    /// <summary>
    /// Number of pages in the document.
    /// </summary>
    public int PageCount => Pages.Count;

    /// <summary>
    /// Export the document in the given format, optionally restricted to a range of pages.
    /// </summary>
    /// <param name="exportFormat">Format configuration for the document export.</param>
    /// <param name="pages">0-based, inclusive page range to export, or <c>null</c> for the whole document.</param>
    /// <exception cref="OicanaException">If the export fails.</exception>
    /// <returns>Stream containing the exported document.</returns>
    public Stream Export(ExportFormat exportFormat, PageRange? pages = null)
    {
        EnsureOpen();
        return OicanaFfi.ExportDocument(_documentId!, exportFormat, pages);
    }

    /// <summary>
    /// Export the document to PDF, optionally restricted to a range of pages.
    /// Tagging will be automatically turned off when exporting a subset of pages.
    /// </summary>
    /// <param name="pages">0-based, inclusive page range to export, or <c>null</c> for the whole document.</param>
    /// <exception cref="OicanaException">If the export fails.</exception>
    /// <returns>Stream containing the exported PDF.</returns>
    public Stream ExportPdf(PageRange? pages = null)
    {
        return Export(ExportFormat.Pdf(), pages);
    }

    /// <summary>
    /// Export the document to PNG, optionally restricted to a range of pages.
    /// Multiple pages are merged into a single, vertically stacked image.
    /// </summary>
    /// <param name="pixelsPerPt">Resolution in pixels per point (defaults to 1.0).</param>
    /// <param name="pages">0-based, inclusive page range to export, or <c>null</c> for the whole document.</param>
    /// <exception cref="OicanaException">If the export fails.</exception>
    /// <returns>Stream containing the exported PNG.</returns>
    public Stream ExportPng(float pixelsPerPt = 1.0f, PageRange? pages = null)
    {
        return Export(ExportFormat.Png(pixelsPerPt), pages);
    }

    /// <summary>
    /// Export the document to SVG, optionally restricted to a range of pages.
    /// </summary>
    /// <param name="pages">0-based, inclusive page range to export, or <c>null</c> for the whole document.</param>
    /// <exception cref="OicanaException">If the export fails.</exception>
    /// <returns>Stream containing the exported SVG.</returns>
    public Stream ExportSvg(PageRange? pages = null)
    {
        return Export(ExportFormat.Svg(), pages);
    }

    /// <summary>
    /// Release the cached document. The instance must not be used after disposal.
    /// </summary>
    public void Dispose()
    {
        if (_documentId != null)
        {
            OicanaFfi.RemoveDocument(_documentId);
            _documentId = null;
        }
    }

    private void EnsureOpen()
    {
        if (_documentId == null)
        {
            throw new ObjectDisposedException(nameof(CompiledDocument));
        }
    }

    private static IReadOnlyList<PageSize> ParsePageSizes(string json)
    {
        var array = JsonNode.Parse(json)?.AsArray() ?? new JsonArray();
        var pages = new List<PageSize>(array.Count);
        foreach (var node in array)
        {
            pages.Add(new PageSize(
                node!["width"]!.GetValue<double>(),
                node["height"]!.GetValue<double>()));
        }

        return pages;
    }
}
