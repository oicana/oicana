namespace Oicana;

/// <summary>
/// A font face made available to templates by the host.
/// </summary>
/// <param name="Family">Family name, as used in Typst's <c>text(font: ...)</c>.</param>
/// <param name="Path">File the face was read from; <c>null</c> for fonts registered from memory.</param>
public record RegisteredFont(string Family, string? Path);
