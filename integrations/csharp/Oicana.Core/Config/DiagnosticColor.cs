namespace Oicana.Config;

/// <summary>
/// Color mode for compilation diagnostics like warnings and errors.
/// </summary>
public enum DiagnosticColor
{
    /// <summary>
    /// No colors in diagnostic output.
    /// </summary>
    None = 0,

    /// <summary>
    /// ANSI codes for colors in diagnostic output.
    /// </summary>
    Ansi = 1,
}
