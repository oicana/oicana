namespace Oicana.Config;

/// <summary>
/// The mode of compilation
/// </summary>
public enum CompilationMode
{
    /// <summary>
    /// Use development values for inputs if an input is not explicitly set.
    /// If there is no development value defined, but a default one, fall back to that.
    /// </summary>
    Development = 0,

    /// <summary>
    /// If an input is not set, use the default value if available.
    /// This mode will never use a development value for an input.
    /// </summary>
    Production = 1
}
