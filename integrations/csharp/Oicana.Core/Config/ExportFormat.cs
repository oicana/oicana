namespace Oicana.Config;

/// <summary>
/// Export format configuration for an Oicana template compilation.
/// </summary>
public class ExportFormat
{
    internal ExportTarget ExportTarget;
    internal float? PixelsPerPt;

    /// <summary>
    /// Create a new export format configuration for compiling to PDF
    /// </summary>
    public static ExportFormat Pdf()
    {
        return new ExportFormat()
        {
            ExportTarget = ExportTarget.Pdf,
        };
    }

    /// <summary>
    /// Create a new export format configuration for compiling to PNG
    /// </summary>
    /// <param name="pixelsPerPt">The number of pixels per pt. Higher numbers take longer, but create sharper images.</param>
    /// <exception cref="ArgumentOutOfRangeException">If <paramref name="pixelsPerPt"/> is not a positive, finite number.</exception>
    public static ExportFormat Png(float pixelsPerPt = 1.0f)
    {
        if (!float.IsFinite(pixelsPerPt) || pixelsPerPt <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(pixelsPerPt), pixelsPerPt,
                "pixelsPerPt must be a positive, finite number.");
        }

        return new ExportFormat()
        {
            ExportTarget = ExportTarget.Png,
            PixelsPerPt = pixelsPerPt
        };
    }

    /// <summary>
    /// Create a new export format configuration for compiling to SVG
    /// </summary>
    public static ExportFormat Svg()
    {
        return new ExportFormat()
        {
            ExportTarget = ExportTarget.Svg,
        };
    }
}
