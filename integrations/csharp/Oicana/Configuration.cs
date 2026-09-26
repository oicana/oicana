using System.Collections.Generic;
using System.Text.Json;
using Oicana.Interop;
using DiagnosticColor = Oicana.Config.DiagnosticColor;

namespace Oicana;

/// <summary>
/// Global Oicana configuration.
/// </summary>
public static class Configuration
{
    /// <summary>
    /// Configure the coloring of compilation diagnostics.
    /// </summary>
    /// <param name="color">Coloring for Oicana diagnostics.</param>
    public static void ConfigureDiagnosticColor(DiagnosticColor color)
    {
        OicanaFfi.Configure(color);
    }

    /// <summary>
    /// Configure automatic cache eviction after each compilation.
    ///
    /// Each cache entry has an age counter that increases by 1 during each eviction
    /// and resets to 0 on cache hit. Entries with age >= maxAge are removed.
    /// </summary>
    /// <param name="maxAge">
    /// Maximum age threshold, or <c>null</c> to disable:
    ///   - null - Disables cache eviction (cache never cleared)
    ///   - 0 - Clears all cache after every compilation
    ///   - 1 - Keeps only entries used since the last eviction
    ///   - n - Keeps entries used within the last n evictions
    /// Default is 10.
    /// </param>
    public static void ConfigureAutomaticCacheEviction(long? maxAge)
    {
        OicanaFfi.ConfigureAutomaticCacheEviction(maxAge);
    }

    /// <summary>
    /// Evict the cache with the given age threshold.
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

    /// <summary>
    /// Make fonts available to every template registered from now on.
    /// </summary>
    /// <param name="fonts">Raw content of the font files. Data that holds no font is ignored.</param>
    /// <returns>The number of font faces that were added.</returns>
    public static long RegisterFonts(params byte[][] fonts)
    {
        long faces = 0;
        foreach (var font in fonts)
        {
            faces += OicanaFfi.RegisterFont(font);
        }

        return faces;
    }

    /// <summary>
    /// Make fonts on disk available to every template registered from now on.
    /// </summary>
    /// <param name="paths">Paths to font files, or to directories whose font files are all added.</param>
    /// <returns>The number of font faces that were added.</returns>
    public static long RegisterFontPaths(params string[] paths)
    {
        long faces = 0;
        foreach (var path in paths)
        {
            faces += OicanaFfi.RegisterFontPath(path);
        }

        return faces;
    }

    /// <summary>
    /// All font faces currently registered by the host.
    /// </summary>
    /// <returns>The registered faces, in registration order.</returns>
    public static IReadOnlyList<RegisteredFont> RegisteredFonts()
    {
        var json = OicanaFfi.RegisteredFonts();
        return JsonSerializer.Deserialize<List<RegisteredFont>>(json, FontSerializerOptions)
               ?? new List<RegisteredFont>();
    }

    /// <summary>
    /// Drop all fonts registered by the host.
    ///
    /// Templates that are already registered keep the fonts they were created with.
    /// </summary>
    public static void ClearFonts()
    {
        OicanaFfi.ClearFonts();
    }

    private static readonly JsonSerializerOptions FontSerializerOptions = new()
    {
        PropertyNameCaseInsensitive = true,
    };
}
