using Oicana.Interop;
using Buffer = Oicana.Interop.Buffer;

namespace Oicana;

/// <summary>
/// A stream reading memory from Rust.
/// </summary>
/// <remarks>
/// Do not hold on to this stream. It will free the Rust memory when disposed.
/// </remarks>
internal class RustMemoryStream : UnmanagedMemoryStream
{
    private readonly Buffer _buffer;
    private bool _isDisposed;

    /// <summary>
    /// Create a new stream for the given buffer
    ///
    /// The buffer will be automatically freed as soon as the stream is disposed of.
    /// </summary>
    /// <param name="buffer">Buffer pointing to allocated Rust memory.</param>
    internal unsafe RustMemoryStream(Buffer buffer)
        : base((byte*)buffer.data.ToPointer(), buffer.len, buffer.len, FileAccess.Read)
    {
        _buffer = buffer;
    }

    /// <summary>
    /// Safety net releasing the Rust memory if the stream was never disposed.
    /// </summary>
    ~RustMemoryStream()
    {
        Dispose(false);
    }

    /// <inheritdoc />
    protected override void Dispose(bool disposing)
    {
        if (!_isDisposed)
        {
            base.Dispose(disposing);
            OicanaFfiInternal.unsafe_free_buffer(_buffer);
            _isDisposed = true;
        }

        if (disposing)
        {
            GC.SuppressFinalize(this);
        }
    }
}
