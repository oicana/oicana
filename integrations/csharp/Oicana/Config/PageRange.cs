namespace Oicana.Config;

/// <summary>
/// A contiguous, 0-based inclusive range of document pages to export.
///
/// Both bounds are optional; leaving one as <c>null</c> keeps it open. For
/// example, <see cref="Of"/> with <c>start: 1</c> selects the second page to the
/// end of the document.
/// </summary>
public class PageRange
{
    internal int? Start;
    internal int? End;

    /// <summary>
    /// Create a range selecting exactly the page at the given 0-based index.
    /// </summary>
    /// <param name="page">The 0-based index of the page to select.</param>
    /// <exception cref="ArgumentOutOfRangeException">If <paramref name="page"/> is negative.</exception>
    public static PageRange Single(int page)
    {
        NonNegative(page, nameof(page));
        return new PageRange()
        {
            Start = page,
            End = page
        };
    }

    /// <summary>
    /// Create a range with the given (nullable) 0-based, inclusive bounds.
    /// </summary>
    /// <param name="start">The first page index to export, or <c>null</c> to start at the first page.</param>
    /// <param name="end">The last page index to export, or <c>null</c> to go to the last page.</param>
    /// <exception cref="ArgumentOutOfRangeException">If a bound is negative.</exception>
    public static PageRange Of(int? start = null, int? end = null)
    {
        NonNegative(start, nameof(start));
        NonNegative(end, nameof(end));
        return new PageRange()
        {
            Start = start,
            End = end
        };
    }

    private static void NonNegative(int? value, string name)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(name, value, "Pages are 0-based indices and cannot be negative.");
        }
    }
}
