namespace Oicana;

/// <summary>
/// Exception while compiling a template.
/// </summary>
public class OicanaException : Exception
{
    /// <summary>
    /// Exception while compiling a template.
    /// </summary>
    /// <param name="error">Error message of the exception.</param>
    public OicanaException(String error) : base(error) { }
}
