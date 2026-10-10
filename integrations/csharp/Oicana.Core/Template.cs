using System.Text.Json.Nodes;
using Oicana.Interop;
using Oicana.Manifest;
using Oicana.Inputs;
using CompilationMode = Oicana.Config.CompilationMode;
using ExportFormat = Oicana.Config.ExportFormat;
using PageRange = Oicana.Config.PageRange;
using ZipLimits = Oicana.Config.ZipLimits;

namespace Oicana;

/// <summary>
/// An Oicana template
///
/// This class caches the template file and prepares fast
/// compilation as part of the constructor. Keep it around
/// to compile the same template with different inputs.
/// </summary>
public class Template : ITemplate, IDisposable
{
    private readonly string _templateId;

    /// <summary>
    /// Prepare a template for fast compilation.
    ///
    /// This will compile the document in development mode. If your template has required inputs without default or development values, use the constructor with inputs.
    ///
    /// This call can be expensive depending on the template.
    /// Reuse instances of this class if possible.
    ///
    /// If you want to compile a template once and not cache the template
    /// use <see cref="ExportOnce"/> instead.
    /// </summary>
    /// <param name="templateFile">The packed Oicana template to register.</param>
    /// <exception cref="OicanaException">If the initial template compilation fails.</exception>
    public Template(byte[] templateFile) : this(templateFile, CompilationMode.Development) { }

    /// <summary>
    /// Prepare a template for fast compilation.
    ///
    /// Your template should not require any explicit input values in
    /// the given compilation mode, otherwise registration will fail.
    ///
    /// This call can be expensive depending on the template.
    /// Reuse instances of this class if possible.
    ///
    /// If you want to compile a template once and not cache the template
    /// use <see cref="ExportOnce"/> instead.
    /// </summary>
    /// <param name="templateFile">The packed Oicana template to register.</param>
    /// <param name="compilationMode">Compilation mode to use for the initial template compilation during registration</param>
    /// <exception cref="OicanaException">If the initial template compilation fails.</exception>
    public Template(byte[] templateFile, CompilationMode compilationMode) : this(templateFile, new Dictionary<string, JsonNode>(), new Dictionary<string, BlobInput>(), compilationMode) { }

    /// <summary>
    /// Prepare a template for fast compilation.
    ///
    /// This call can be expensive depending on the template.
    /// Reuse instances of this class if possible.
    ///
    /// If you want to compile a template once and not cache the template
    /// use <see cref="ExportOnce"/> instead.
    /// </summary>
    /// <param name="templateFile">The packed Oicana template to register.</param>
    /// <param name="jsonInputs">Json inputs for the initial compilation (key -> JsonNode), or <c>null</c> for none.</param>
    /// <param name="blobInputs">Blob inputs for the initial compilation (key -> BlobInput), or <c>null</c> for none.</param>
    /// <param name="compilationMode">Compilation mode to use for the initial template compilation during registration.</param>
    /// <param name="limits">Limits for reading the packed template zip, or <c>null</c> for the defaults.</param>
    /// <exception cref="OicanaException">If the initial template compilation fails.</exception>
    public Template(byte[] templateFile, IDictionary<string, JsonNode>? jsonInputs = null, IDictionary<string, BlobInput>? blobInputs = null, CompilationMode compilationMode = CompilationMode.Development, ZipLimits? limits = null)
    {
        _templateId = Guid.NewGuid().ToString();
        using var documentIdStream = OicanaFfi.RegisterTemplate(_templateId, templateFile, jsonInputs ?? EmptyJsonInputs, blobInputs ?? EmptyBlobInputs, compilationMode, limits);
        var documentId = OicanaFfi.GetMessageFromStream(documentIdStream);
        Warnings = OicanaFfi.GetWarnings(documentId);
        OicanaFfi.RemoveDocument(documentId);
    }

    private static readonly IDictionary<string, JsonNode> EmptyJsonInputs =
        new Dictionary<string, JsonNode>();

    private static readonly IDictionary<string, BlobInput> EmptyBlobInputs =
        new Dictionary<string, BlobInput>();

    /// <inheritdoc />
    public string? Warnings { get; private set; }

    /// <inheritdoc />
    public Stream Export(IDictionary<string, JsonNode>? jsonInputs = null, IDictionary<string, BlobInput>? blobInputs = null, ExportFormat? exportFormat = null, CompilationMode mode = CompilationMode.Production, PageRange? pages = null)
    {
        var documentId = OicanaFfi.CompileTemplate(_templateId, jsonInputs ?? EmptyJsonInputs, blobInputs ?? EmptyBlobInputs, mode);
        Warnings = OicanaFfi.GetWarnings(documentId);
        try
        {
            return OicanaFfi.ExportDocument(documentId, exportFormat ?? ExportFormat.Pdf(), pages);
        }
        finally
        {
            OicanaFfi.RemoveDocument(documentId);
        }
    }

    /// <inheritdoc />
    public Stream ExportPdf(IDictionary<string, JsonNode>? jsonInputs = null, IDictionary<string, BlobInput>? blobInputs = null, CompilationMode mode = CompilationMode.Production, PageRange? pages = null)
    {
        return Export(jsonInputs, blobInputs, ExportFormat.Pdf(), mode, pages);
    }

    /// <inheritdoc />
    public Stream ExportPng(IDictionary<string, JsonNode>? jsonInputs = null, IDictionary<string, BlobInput>? blobInputs = null, CompilationMode mode = CompilationMode.Production, float pixelsPerPt = 1.0f, PageRange? pages = null)
    {
        return Export(jsonInputs, blobInputs, ExportFormat.Png(pixelsPerPt), mode, pages);
    }

    /// <inheritdoc />
    public Stream ExportSvg(IDictionary<string, JsonNode>? jsonInputs = null, IDictionary<string, BlobInput>? blobInputs = null, CompilationMode mode = CompilationMode.Production, PageRange? pages = null)
    {
        return Export(jsonInputs, blobInputs, ExportFormat.Svg(), mode, pages);
    }

    /// <inheritdoc />
    public CompiledDocument Compile(IDictionary<string, JsonNode>? jsonInputs = null, IDictionary<string, BlobInput>? blobInputs = null, CompilationMode mode = CompilationMode.Production)
    {
        var documentId = OicanaFfi.CompileTemplate(_templateId, jsonInputs ?? EmptyJsonInputs, blobInputs ?? EmptyBlobInputs, mode);
        var document = new CompiledDocument(documentId);
        Warnings = document.Warnings;
        return document;
    }

    /// <summary>
    /// Compile the given template once.
    /// </summary>
    /// <remarks>
    /// If you want to compile the same document multiple times with different input values,
    /// create an instance of <see cref="Template"/> and use <see cref="Export"/> instead.
    ///
    /// <see cref="ExportOnce"/> does not cache the template or document and compiles from scratch, which is slower than exporting a prepared template.
    /// </remarks>
    /// <param name="templateFile">The packed Oicana template to compile.</param>
    /// <param name="jsonInputs">Json inputs for the compilation (key -> JsonNode).</param>
    /// <param name="blobInputs">Blob inputs for the compilation (key -> BlobInput).</param>
    /// <param name="exportFormat">Format configuration for the document export.</param>
    /// <param name="mode">Mode to compile the template in (defaults to <c>Production</c>).</param>
    /// <param name="pages">0-based, inclusive page range to export, or <c>null</c> for the whole document.</param>
    /// <param name="limits">Limits for reading the packed template zip, or <c>null</c> for the defaults.</param>
    /// <exception cref="OicanaException">If the template compilation fails.</exception>
    /// <returns>The exported document and any compilation warnings.</returns>
    public static ExportOnceResult ExportOnce(byte[] templateFile, IDictionary<string, JsonNode>? jsonInputs = null, IDictionary<string, BlobInput>? blobInputs = null, ExportFormat? exportFormat = null, CompilationMode mode = CompilationMode.Production, PageRange? pages = null, ZipLimits? limits = null)
    {
        return OicanaFfi.ExportTemplateOnce(templateFile, jsonInputs ?? EmptyJsonInputs, blobInputs ?? EmptyBlobInputs, mode, exportFormat ?? ExportFormat.Pdf(), pages, limits);
    }

    /// <inheritdoc />
    public TemplateManifest Manifest()
    {
        return TemplateManifest.FromJson(OicanaFfi.GetManifest(_templateId));
    }

    /// <inheritdoc />
    public string Source(string path)
    {
        return OicanaFfi.GetSource(_templateId, path);
    }

    /// <inheritdoc />
    public byte[] File(string path)
    {
        return OicanaFfi.GetFile(_templateId, path);
    }

    /// <summary>
    /// Enable or disable JSON schema validation for this template.
    ///
    /// When enabled (the default), JSON inputs are validated against their schemas
    /// before compilation.
    /// </summary>
    /// <param name="validate">Whether to validate inputs against their JSON schemas.</param>
    public void SetValidateInputs(bool validate)
    {
        OicanaFfi.SetValidateInputs(_templateId, validate);
    }

    /// <inheritdoc/>
    public void Dispose()
    {
        OicanaFfi.ResetTemplate(_templateId);
    }

    /// <inheritdoc/>
    public override string ToString() =>
        $"Template {_templateId}";
}
