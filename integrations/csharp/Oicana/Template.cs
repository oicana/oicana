using System.Text.Json.Nodes;
using Oicana.Interop;
using Oicana.Manifest;
using Oicana.Inputs;
using CompilationMode = Oicana.Config.CompilationMode;
using CompilationOptions = Oicana.Config.CompilationOptions;
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
    /// This will compile the document in development mode.
    /// Your template should not have any required inputs that don't have
    /// development or default values defined, otherwise registration will fail.
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
    /// <param name="jsonInputs">Json inputs for the initial compilation (key -> JsonNode).</param>
    /// <param name="blobInputs">Blob inputs for the initial compilation (key -> BlobInput).</param>
    /// <param name="compilationMode">Compilation mode to use for the initial template compilation during registration.</param>
    /// <param name="limits">Limits for reading the packed template zip, or <c>null</c> for the defaults.</param>
    /// <exception cref="OicanaException">If the initial template compilation fails.</exception>
    public Template(byte[] templateFile, IDictionary<string, JsonNode> jsonInputs, IDictionary<string, BlobInput> blobInputs, CompilationMode compilationMode, ZipLimits? limits = null)
    {
        _templateId = Guid.NewGuid().ToString();
        using var documentIdStream = OicanaFfi.RegisterTemplate(_templateId, templateFile, jsonInputs, blobInputs, new CompilationOptions(compilationMode), limits);
        var documentId = OicanaFfi.GetMessageFromStream(documentIdStream);
        Warnings = OicanaFfi.GetWarnings(documentId);
        OicanaFfi.RemoveDocument(documentId);
    }

    /// <inheritdoc />
    public string? Warnings { get; private set; }

    /// <inheritdoc />
    public Stream Export(IDictionary<string, JsonNode> jsonInputs, IDictionary<string, BlobInput> blobInputs, ExportFormat exportFormat, CompilationOptions compilationOptions, PageRange? pages = null)
    {
        var documentId = OicanaFfi.CompileTemplate(_templateId, jsonInputs, blobInputs, compilationOptions);
        Warnings = OicanaFfi.GetWarnings(documentId);
        try
        {
            return OicanaFfi.ExportDocument(documentId, exportFormat, pages);
        }
        finally
        {
            OicanaFfi.RemoveDocument(documentId);
        }
    }

    /// <inheritdoc />
    public Stream ExportPdf(IDictionary<string, JsonNode> jsonInputs, IDictionary<string, BlobInput> blobInputs, CompilationOptions compilationOptions, PageRange? pages = null)
    {
        return Export(jsonInputs, blobInputs, ExportFormat.Pdf(), compilationOptions, pages);
    }

    /// <inheritdoc />
    public Stream ExportPng(IDictionary<string, JsonNode> jsonInputs, IDictionary<string, BlobInput> blobInputs, CompilationOptions compilationOptions, float pixelsPerPt = 1.0f, PageRange? pages = null)
    {
        return Export(jsonInputs, blobInputs, ExportFormat.Png(pixelsPerPt), compilationOptions, pages);
    }

    /// <inheritdoc />
    public Stream ExportSvg(IDictionary<string, JsonNode> jsonInputs, IDictionary<string, BlobInput> blobInputs, CompilationOptions compilationOptions, PageRange? pages = null)
    {
        return Export(jsonInputs, blobInputs, ExportFormat.Svg(), compilationOptions, pages);
    }

    /// <inheritdoc />
    public CompiledDocument Compile(IDictionary<string, JsonNode> jsonInputs, IDictionary<string, BlobInput> blobInputs, CompilationOptions compilationOptions)
    {
        var documentId = OicanaFfi.CompileTemplate(_templateId, jsonInputs, blobInputs, compilationOptions);
        var document = new CompiledDocument(documentId);
        Warnings = document.Warnings;
        return document;
    }

    /// <summary>
    /// Compile the given template once.
    /// </summary>
    /// <remarks>
    /// If you want to compile the same document multiple times with different input values,
    /// create an instance of <see cref="Template"/> and use <see cref="Export(IDictionary{string, JsonNode}, IDictionary{string, BlobInput}, ExportFormat, CompilationOptions, PageRange)"/> instead.
    ///
    /// <see cref="ExportOnce"/> does not cache the template or document and compiles from scratch, which is slower than exporting a prepared template.
    /// </remarks>
    /// <param name="templateFile">The packed Oicana template to compile.</param>
    /// <param name="jsonInputs">Json inputs for the compilation (key -> JsonNode).</param>
    /// <param name="blobInputs">Blob inputs for the compilation (key -> BlobInput).</param>
    /// <param name="exportFormat">Format configuration for the document export.</param>
    /// <param name="compilationOptions">Options for the template compilation.</param>
    /// <param name="pages">0-based, inclusive page range to export, or <c>null</c> for the whole document.</param>
    /// <param name="limits">Limits for reading the packed template zip, or <c>null</c> for the defaults.</param>
    /// <exception cref="OicanaException">If the template compilation fails.</exception>
    /// <returns>The exported document and any compilation warnings.</returns>
    public static ExportOnceResult ExportOnce(byte[] templateFile, IDictionary<string, JsonNode> jsonInputs, IDictionary<string, BlobInput> blobInputs, ExportFormat exportFormat, CompilationOptions compilationOptions, PageRange? pages = null, ZipLimits? limits = null)
    {
        return OicanaFfi.ExportTemplateOnce(templateFile, jsonInputs, blobInputs, compilationOptions, exportFormat, pages, limits);
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

    /// <summary>
    /// Manually evict the cache with the given age threshold.
    /// </summary>
    /// <param name="maxAge">
    /// Maximum age threshold for eviction.
    /// Entries with age >= this value will be removed.
    /// Calls with negative maxAge are ignored.
    /// </param>
    public static void EvictCache(long maxAge)
    {
        OicanaFfi.EvictCache(maxAge);
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
