using System.Runtime.InteropServices;
using System.Text;
using System.Text.Json.Nodes;
using Oicana.Config;
using Oicana.Inputs;

namespace Oicana.Interop;

/// <summary>
/// Compile Oicana templates
/// </summary>
internal static class OicanaFfi
{
    /// <summary>
    /// Compile the given template once and do not cache anything
    ///
    /// This method does a clean compile which can take significantly longer
    /// than compiling a template through the `Template` class.
    /// If you want to compile a template multiple times with different
    /// inputs use the Template class!
    /// </summary>
    /// <param name="templateFile">The packed Oicana template to compile.</param>
    /// <param name="jsonInputs">Json inputs for the compilation (key -> JsonNode).</param>
    /// <param name="blobInputs">Blob inputs for the compilation (key -> BlobInput).</param>
    /// <param name="compilationOptions">Options for the template compilation.</param>
    /// <param name="exportFormat">Format configuration for the document export.</param>
    /// <param name="pages">0-based, inclusive page range to export, or <c>null</c> for the whole document.</param>
    /// <param name="limits">Limits for reading the packed template zip, or <c>null</c> to use the defaults.</param>
    /// <exception cref="OicanaException">If the template compilation fails.</exception>
    /// <returns>The exported document and any compilation warnings.</returns>
    public static ExportOnceResult ExportTemplateOnce(byte[] templateFile, IDictionary<string, JsonNode> jsonInputs, IDictionary<string, BlobInput> blobInputs, Oicana.Config.CompilationOptions compilationOptions, Oicana.Config.ExportFormat exportFormat, Oicana.Config.PageRange? pages = null, Oicana.Config.ZipLimits? limits = null)
    {
        GCHandle fileHandle = GCHandle.Alloc(templateFile, GCHandleType.Pinned);
        try
        {
            IntPtr filePointer = fileHandle.AddrOfPinnedObject();
            var fileBuffer = new Buffer() { data = filePointer, error = false, len = (uint)templateFile.Length };

            PreparedInputs preparedInputs = PrepareInputs(jsonInputs, blobInputs);
            try
            {
                var buffers = OicanaFfiInternal.unsafe_export_template_once(fileBuffer, preparedInputs.JsonInputs, preparedInputs.BlobInputs, ConvertCompileOptions(compilationOptions), ConvertExportFormat(exportFormat), ConvertPageRange(pages), ConvertZipLimits(limits));
                var warnings = GetStringFromBuffer(buffers.warnings);
                var document = HandleBuffer(buffers.document);
                return new ExportOnceResult(document, warnings.Length == 0 ? null : warnings);
            }
            finally
            {
                preparedInputs.FreeAll();
            }
        }
        finally
        {
            fileHandle.Free();
        }
    }

    /// <summary>
    /// Compile a template with the given id and inputs and export it to the specified <see cref="ExportTarget"/>.
    /// </summary>
    /// <param name="templateId">Identifier of the template for the internal cache.</param>
    /// <param name="jsonInputs">Json inputs for the compilation (key -> JsonNode).</param>
    /// <param name="blobInputs">Blob inputs for the compilation (key -> BlobInput).</param>
    /// <param name="compilationOptions">Options for the template compilation.</param>
    /// <exception cref="OicanaException">If the template compilation fails.</exception>
    /// <returns>Stream containing the compiled template exported as the given <see cref="ExportTarget"/>.</returns>
    public static String CompileTemplate(string templateId, IDictionary<string, JsonNode> jsonInputs, IDictionary<string, BlobInput> blobInputs, Oicana.Config.CompilationOptions compilationOptions)
    {
        PreparedInputs preparedInputs = PrepareInputs(jsonInputs, blobInputs);
        try
        {
            var buffer = OicanaFfiInternal.unsafe_compile_template(templateId, preparedInputs.JsonInputs, preparedInputs.BlobInputs, ConvertCompileOptions(compilationOptions));
            return HandleStringBuffer(buffer);
        }
        finally
        {
            preparedInputs.FreeAll();
        }
    }

    /// <summary>
    /// Register and compile a template with the given id and inputs and export it to the specified <see cref="ExportTarget"/>.
    /// </summary>
    /// <param name="templateId">Identifier of the template for the internal cache.</param>
    /// <param name="templateFile">The packed Oicana template to compile.</param>
    /// <param name="jsonInputs">Json inputs for the compilation (key -> JsonNode).</param>
    /// <param name="blobInputs">Blob inputs for the compilation (key -> BlobInput).</param>
    /// <param name="compilationOptions">Options for the template compilation.</param>
    /// <param name="limits">Limits for reading the packed template zip, or <c>null</c> for the defaults.</param>
    /// <exception cref="OicanaException">If the template compilation fails.</exception>
    /// <returns>Stream containing the compiled template exported as the given <see cref="ExportTarget"/>.</returns>
    public static Stream RegisterTemplate(string templateId, byte[] templateFile, IDictionary<string, JsonNode> jsonInputs, IDictionary<string, BlobInput> blobInputs, Oicana.Config.CompilationOptions compilationOptions, Oicana.Config.ZipLimits? limits = null)
    {
        GCHandle fileHandle = GCHandle.Alloc(templateFile, GCHandleType.Pinned);
        try
        {
            IntPtr filePointer = fileHandle.AddrOfPinnedObject();
            var fileBuffer = new Buffer() { data = filePointer, error = false, len = (uint)templateFile.Length };

            PreparedInputs preparedInputs = PrepareInputs(jsonInputs, blobInputs);
            try
            {
                var buffer = OicanaFfiInternal.unsafe_register_template(templateId, fileBuffer, preparedInputs.JsonInputs, preparedInputs.BlobInputs, ConvertCompileOptions(compilationOptions), ConvertZipLimits(limits));
                return HandleBuffer(buffer);
            }
            finally
            {
                preparedInputs.FreeAll();
            }
        }
        finally
        {
            fileHandle.Free();
        }
    }

    /// <summary>
    /// Export the given document
    ///
    /// This method requires the document to be in the internal cache.
    /// After the export it will not be removed from the cache automatically. It's the callers
    /// responsibility to free the documents memory when no more exports are needed by calling
    /// `RemoveDocument`.
    /// </summary>
    /// <param name="documentId">Id of document to export.</param>
    /// <param name="exportFormat">Format configuration for the document export.</param>
    /// <param name="pages">0-based, inclusive page range to export, or <c>null</c> for the whole document.</param>
    /// <exception cref="OicanaException">If the template compilation fails.</exception>
    /// <returns>Stream containing the compiled template exported as the given <see cref="ExportTarget"/>.</returns>
    public static Stream ExportDocument(string documentId, Oicana.Config.ExportFormat exportFormat, Oicana.Config.PageRange? pages = null)
    {
        var buffer = OicanaFfiInternal.unsafe_export_document(documentId, ConvertExportFormat(exportFormat), ConvertPageRange(pages));

        return HandleBuffer(buffer);
    }

    /// <summary>
    /// Reset the world cache of the given template id
    /// </summary>
    /// <param name="id">The identifier of the template to reset.</param>
    public static void ResetTemplate(string id)
    {
        OicanaFfiInternal.remove_world(id);
    }

    /// <summary>
    /// Remove a document from the internal cache
    /// </summary>
    /// <param name="documentId">The identifier of the document to remove from the cache.</param>
    public static void RemoveDocument(string documentId)
    {
        OicanaFfiInternal.remove_document(documentId);
    }

    /// <summary>
    /// Set the global cache age for comemo cache eviction.
    ///
    /// # How Cache Aging Works
    ///
    /// - Each cache entry has an age counter
    /// - Age increases by 1 during each eviction call
    /// - Age resets to 0 when the entry is used (cache hit)
    /// - Entries with age >= `maxAge` are removed
    ///
    /// Default: 10
    /// </summary>
    /// <param name="maxAge">
    /// Maximum age threshold, or -1 to disable:
    ///   - `-1` - Disables cache eviction (cache never cleared)
    ///   - `0` - Clears all cache after every compilation
    ///   - `1` - Keeps only entries used since the last eviction
    ///   - `n` - Keeps entries used within the last n evictions
    /// </param>
    public static void ConfigureAutomaticCacheEviction(long maxAge)
    {
        OicanaFfiInternal.configure_automatic_cache_eviction(maxAge);
    }

    /// <summary>
    /// Manually evict the comemo cache based on the configured cache age.
    /// </summary>
    /// <param name="maxAge">
    /// Maximum age threshold for cache eviction.
    /// Entries with age >= this value will be removed.
    /// Pass -1 to disable eviction.
    /// </param>
    public static void EvictCache(long maxAge)
    {
        OicanaFfiInternal.evict_cache(maxAge);
    }

    /// <summary>
    /// Register a single font from its raw file content.
    /// </summary>
    /// <param name="font">Raw content of a font file.</param>
    /// <returns>The number of font faces that were added; 0 if the data held no font.</returns>
    public static long RegisterFont(byte[] font)
    {
        GCHandle fontHandle = GCHandle.Alloc(font, GCHandleType.Pinned);
        try
        {
            IntPtr fontPointer = fontHandle.AddrOfPinnedObject();
            var fontBuffer = new Buffer() { data = fontPointer, error = false, len = (uint)font.Length };
            return OicanaFfiInternal.unsafe_register_font(fontBuffer);
        }
        finally
        {
            fontHandle.Free();
        }
    }

    /// <summary>
    /// Register a single font file by path, not retaining its data until it is used.
    /// </summary>
    /// <param name="path">Path to a font file.</param>
    /// <returns>The number of font faces that were added; 0 if the file could not be read or held no font.</returns>
    public static long RegisterFontPath(string path)
    {
        return OicanaFfiInternal.register_font_path(path);
    }

    /// <summary>
    /// Get all font faces currently registered by the host.
    /// </summary>
    /// <exception cref="OicanaException">If the registered fonts cannot be serialized.</exception>
    /// <returns>JSON array of <c>{ "family": string, "path": string | null }</c>.</returns>
    public static string RegisteredFonts()
    {
        var buffer = OicanaFfiInternal.registered_fonts();
        return HandleStringBuffer(buffer);
    }

    /// <summary>
    /// Drop all fonts registered by the host.
    /// </summary>
    public static void ClearFonts()
    {
        OicanaFfiInternal.clear_fonts();
    }

    /// <summary>
    /// Get the manifest of the template.
    /// </summary>
    /// <param name="templateId">Identifier of the template.</param>
    /// <exception cref="OicanaException">If the template is not registered or the manifest cannot be retrieved.</exception>
    /// <returns>JSON string containing the manifest.</returns>
    public static string GetManifest(string templateId)
    {
        var buffer = OicanaFfiInternal.manifest(templateId);
        return HandleStringBuffer(buffer);
    }

    /// <summary>
    /// Get the sizes (in points) of every page of a compiled document.
    /// </summary>
    /// <param name="documentId">Identifier of the compiled document.</param>
    /// <exception cref="OicanaException">If the document is not found.</exception>
    /// <returns>JSON array of <c>{ "width": number, "height": number }</c> in points.</returns>
    public static string DocumentPages(string documentId)
    {
        var buffer = OicanaFfiInternal.document_pages(documentId);
        return HandleStringBuffer(buffer);
    }

    /// <summary>
    /// Get the compilation warnings produced for the given document.
    /// </summary>
    /// <param name="documentId">Identifier of the compiled document.</param>
    /// <returns>The warnings text, or <c>null</c> if there were none.</returns>
    public static string? GetWarnings(string documentId)
    {
        var buffer = OicanaFfiInternal.get_warnings(documentId);
        var warnings = HandleStringBuffer(buffer);
        return warnings.Length == 0 ? null : warnings;
    }

    /// <summary>
    /// Get source file content from the template.
    /// </summary>
    /// <param name="templateId">Identifier of the template.</param>
    /// <param name="path">File path in the template.</param>
    /// <exception cref="OicanaException">If the template is not registered or the file cannot be found.</exception>
    /// <returns>Source file content as a string.</returns>
    public static string GetSource(string templateId, string path)
    {
        var buffer = OicanaFfiInternal.get_source(templateId, path);
        return HandleStringBuffer(buffer);
    }

    /// <summary>
    /// Get binary file content from the template.
    /// </summary>
    /// <param name="templateId">Identifier of the template.</param>
    /// <param name="path">File path in the template.</param>
    /// <exception cref="OicanaException">If the template is not registered or the file cannot be found.</exception>
    /// <returns>Binary file content as a byte array.</returns>
    public static byte[] GetFile(string templateId, string path)
    {
        var buffer = OicanaFfiInternal.get_file(templateId, path);
        return HandleByteBuffer(buffer);
    }

    /// <summary>
    /// Enable or disable JSON schema validation for the given template.
    /// </summary>
    /// <param name="templateId">Identifier of the template.</param>
    /// <param name="validate">Whether to validate inputs against their JSON schemas.</param>
    /// <exception cref="OicanaException">If the template is not registered.</exception>
    public static void SetValidateInputs(string templateId, bool validate)
    {
        var buffer = OicanaFfiInternal.set_validate_inputs(templateId, validate);
        if (buffer.error)
        {
            var message = GetStringFromBuffer(buffer);
            throw new OicanaException(message);
        }
    }

    /// <summary>
    /// Configure Oicana.
    /// </summary>
    /// <param name="coloring">Coloring for Oicana diagnostics.</param>
    public static void Configure(DiagnosticsColoring coloring)
    {
        DiagnosticColor color;
        switch (coloring)
        {
            case DiagnosticsColoring.Ansi:
                {
                    color = DiagnosticColor.Ansi;
                }
                break;
            default:
                {
                    color = DiagnosticColor.None;
                }
                break;
        }

        OicanaFfiInternal.configure(new Config
        {
            color = color,
        });
    }

    private record PreparedInputs(IntPtr JsonInputsPtr, int JsonInputCount, SliceFfiJsonInput JsonInputs, IntPtr BlobsInputsPtr, int BlobInputCount, SliceFfiBlobInput BlobInputs, List<GCHandle> BlobHandles)
    {
        internal readonly IntPtr JsonInputsPtr = JsonInputsPtr;
        internal readonly int JsonInputCount = JsonInputCount;
        internal readonly SliceFfiJsonInput JsonInputs = JsonInputs;

        internal readonly IntPtr BlobsInputsPtr = BlobsInputsPtr;
        internal readonly int BlobInputCount = BlobInputCount;
        internal readonly SliceFfiBlobInput BlobInputs = BlobInputs;
        internal readonly List<GCHandle> BlobHandles = BlobHandles;

        internal void FreeAll()
        {
            var jsonInputSize = Marshal.SizeOf<FfiJsonInput>();
            for (int i = 0; i < JsonInputCount; i++)
            {
                Marshal.DestroyStructure<FfiJsonInput>(JsonInputsPtr + i * jsonInputSize);
            }
            Marshal.FreeHGlobal(JsonInputsPtr);

            var blobInputSize = Marshal.SizeOf<FfiBlobInput>();
            for (int i = 0; i < BlobInputCount; i++)
            {
                Marshal.DestroyStructure<FfiBlobInput>(BlobsInputsPtr + i * blobInputSize);
            }
            Marshal.FreeHGlobal(BlobsInputsPtr);

            foreach (var handle in BlobHandles)
            {
                handle.Free();
            }
        }
    }

    internal static Oicana.Interop.CompilationMode ConvertCompilationMode(Oicana.Config.CompilationMode compilationMode)
    {
        switch (compilationMode)
        {
            case Oicana.Config.CompilationMode.Development:
                return Oicana.Interop.CompilationMode.Development;
            case Oicana.Config.CompilationMode.Production:
                return Oicana.Interop.CompilationMode.Production;
        }
        throw new ArgumentException($"The compilation mode {nameof(compilationMode)} is not supported.");
    }

    internal static Oicana.Interop.CompilationOptions ConvertCompileOptions(
        Oicana.Config.CompilationOptions compilationOptions)
    {
        return new CompilationOptions()
        {
            mode = ConvertCompilationMode(compilationOptions.CompilationMode),
        };
    }

    internal static Oicana.Interop.ExportOptions ConvertExportFormat(
        Oicana.Config.ExportFormat exportFormat)
    {
        return new ExportOptions()
        {
            target = ConvertCompileTarget(exportFormat.ExportTarget),
            px_per_pt = exportFormat.PixelsPerPt ?? 1.0f,
        };
    }

    internal static Oicana.Interop.FfiPageRange ConvertPageRange(Oicana.Config.PageRange? pages)
    {
        // A bound of -1 is "open"; { -1, -1 } therefore means "the whole document".
        return new FfiPageRange()
        {
            start = pages?.Start ?? -1,
            end = pages?.End ?? -1,
        };
    }

    internal static Oicana.Interop.FfiZipLimits ConvertZipLimits(Oicana.Config.ZipLimits? limits)
    {
        // A value of -1 keeps the default limit.
        return new FfiZipLimits()
        {
            max_entries = limits?.MaxEntries ?? -1,
            max_total_decompressed_bytes = limits?.MaxTotalDecompressedBytes ?? -1,
        };
    }

    internal static Oicana.Interop.CompilationTarget ConvertCompileTarget(Oicana.Config.ExportTarget exportTarget)
    {
        switch (exportTarget)
        {
            case Oicana.Config.ExportTarget.Pdf:
                return Oicana.Interop.CompilationTarget.Pdf;
            case Oicana.Config.ExportTarget.Png:
                return Oicana.Interop.CompilationTarget.Png;
            case Oicana.Config.ExportTarget.Svg:
                return Oicana.Interop.CompilationTarget.Svg;
        }
        throw new ArgumentException($"The compile target {nameof(exportTarget)} is not supported.");
    }

    private static PreparedInputs PrepareInputs(IDictionary<string, JsonNode> jsonInputs,
        IDictionary<string, BlobInput> blobInputs)
    {
        IntPtr blobsInputsPtr = PrepareBlobInputs(blobInputs, out var blobHandles);
        var blobs = new SliceFfiBlobInput(blobsInputsPtr, (ulong)blobInputs.Count);

        IntPtr inputsPtr = PrepareJsonInputs(jsonInputs);
        var inputs = new SliceFfiJsonInput(inputsPtr, (ulong)jsonInputs.Count);

        return new PreparedInputs(inputsPtr, jsonInputs.Count, inputs, blobsInputsPtr, blobInputs.Count, blobs, blobHandles);
    }

    private static IntPtr PrepareBlobInputs(IDictionary<string, BlobInput> blobs, out List<GCHandle> blobHandles)
    {
        blobHandles = new List<GCHandle>();
        var blobsInputsPtr = Marshal.AllocHGlobal(blobs.Count * Marshal.SizeOf<FfiBlobInput>());
        int i = 0;
        foreach (var (key, blob) in blobs)
        {
            GCHandle blobHandle = GCHandle.Alloc(blob.Data, GCHandleType.Pinned);
            IntPtr dataPtr = blobHandle.AddrOfPinnedObject();
            blobHandles.Add(blobHandle);

            var blobInput = new FfiBlobInput() { key = key, data = new Buffer() { data = dataPtr, error = false, len = (uint)blob.Data.Length }, meta = blob.Metadata?.ToString() ?? "{}" };
            Marshal.StructureToPtr(blobInput, blobsInputsPtr + i * Marshal.SizeOf<FfiBlobInput>(), false);
            i++;
        }

        return blobsInputsPtr;
    }

    private static IntPtr PrepareJsonInputs(IDictionary<string, JsonNode> inputs)
    {
        var inputsPtr = Marshal.AllocHGlobal(inputs.Count * Marshal.SizeOf<FfiJsonInput>());
        int i = 0;
        foreach (var (key, value) in inputs)
        {
            FfiJsonInput jsonInput = new FfiJsonInput { data = value.ToString(), key = key };
            Marshal.StructureToPtr(jsonInput, inputsPtr + i * Marshal.SizeOf<FfiJsonInput>(), false);
            i++;
        }

        return inputsPtr;
    }

    private static String HandleStringBuffer(Buffer buffer)
    {
        var message = GetStringFromBuffer(buffer);

        if (buffer.error)
        {
            throw new OicanaException(message);
        }

        return message;
    }

    private static Stream HandleBuffer(Buffer buffer)
    {
        if (buffer.error)
        {
            unsafe
            {
                UnmanagedMemoryStream errorStream = new UnmanagedMemoryStream((byte*)buffer.data.ToPointer(), buffer.len,
                    buffer.len, FileAccess.Read);
                var error = GetMessageFromStream(errorStream);
                OicanaFfiInternal.unsafe_free_buffer(buffer);
                throw new OicanaException(error);
            }
        }

        return new RustMemoryStream(buffer);
    }

    private static byte[] HandleByteBuffer(Buffer buffer)
    {
        if (buffer.error)
        {
            var message = GetStringFromBuffer(buffer);
            throw new OicanaException(message);
        }

        unsafe
        {
            byte[] result = new byte[buffer.len];
            Marshal.Copy(buffer.data, result, 0, (int)buffer.len);
            OicanaFfiInternal.unsafe_free_buffer(buffer);
            return result;
        }
    }

    public static string GetStringFromBuffer(Buffer buffer)
    {
        unsafe
        {
            try
            {
                UnmanagedMemoryStream stream = new UnmanagedMemoryStream((byte*)buffer.data.ToPointer(),
                    buffer.len,
                    buffer.len, FileAccess.Read);
                return GetMessageFromStream(stream);
            }
            catch (Exception ex)
            {
                return $"Failed to get string from Rust: {ex.Message}";
            }
            finally
            {
                OicanaFfiInternal.unsafe_free_buffer(buffer);
            }
        }
    }

    public static string GetMessageFromStream(Stream stream)
    {
        try
        {
            stream.Seek(0, SeekOrigin.Begin);
            byte[] buffer = new byte[stream.Length];
            stream.ReadExactly(buffer, 0, (int)stream.Length);
            return Encoding.UTF8.GetString(buffer);
        }
        catch (Exception ex)
        {
            return $"Unknown error during template compilation. Failed to read error message: {ex.Message}";
        }
    }
}
