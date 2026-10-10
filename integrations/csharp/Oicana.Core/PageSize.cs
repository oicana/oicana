namespace Oicana;

/// <summary>
/// Size of a single document page, in typographic points (pt).
/// </summary>
/// <param name="Width">Page width in points.</param>
/// <param name="Height">Page height in points.</param>
public sealed record PageSize(double Width, double Height);
