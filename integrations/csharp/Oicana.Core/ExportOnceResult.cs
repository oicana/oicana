namespace Oicana;

/// <summary>
/// Result of a one-shot template export.
/// </summary>
/// <param name="Document">The exported document.</param>
/// <param name="Warnings">Compilation warnings, or <c>null</c> if there were none.</param>
public record ExportOnceResult(Stream Document, string? Warnings);
