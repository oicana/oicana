namespace Oicana.Config;

/// <summary>
/// Limits applied when reading a packed template zip.
///
/// Leaving a bound as <c>null</c> keeps the default
/// (10 000 entries / 512 MiB decompressed).
/// </summary>
public class ZipLimits
{
    private readonly long? _maxEntries;
    private readonly long? _maxTotalDecompressedBytes;

    /// <summary>Maximum number of zip entries, or <c>null</c> for the default.</summary>
    /// <exception cref="ArgumentOutOfRangeException">If the value is negative.</exception>
    public long? MaxEntries
    {
        get => _maxEntries;
        init => _maxEntries = NonNegative(value, nameof(MaxEntries));
    }

    /// <summary>Maximum total decompressed size in bytes, or <c>null</c> for the default.</summary>
    /// <exception cref="ArgumentOutOfRangeException">If the value is negative.</exception>
    public long? MaxTotalDecompressedBytes
    {
        get => _maxTotalDecompressedBytes;
        init => _maxTotalDecompressedBytes = NonNegative(value, nameof(MaxTotalDecompressedBytes));
    }

    private static long? NonNegative(long? value, string name)
    {
        if (value < 0)
        {
            throw new ArgumentOutOfRangeException(name, value, $"{name} must not be negative.");
        }

        return value;
    }
}
