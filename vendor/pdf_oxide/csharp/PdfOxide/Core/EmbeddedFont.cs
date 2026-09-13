using System;
using System.IO;
using PdfOxide.Exceptions;
using PdfOxide.Internal;

namespace PdfOxide.Core
{
    /// <summary>
    /// A TTF / OTF font file that can be registered with a <see cref="DocumentBuilder"/>
    /// so that Unicode text (CJK, Cyrillic, Greek, Hebrew, Arabic, …) renders
    /// correctly through the PDF Type-0 / CIDFontType2 pipeline.
    /// </summary>
    /// <remarks>
    /// <para>
    /// This is a one-shot handle: once passed to
    /// <see cref="DocumentBuilder.RegisterEmbeddedFont(string, EmbeddedFont)"/>,
    /// the underlying native font is moved into the builder's registry and
    /// this wrapper is left in a disposed state. Reusing the same
    /// <see cref="EmbeddedFont"/> after registration throws
    /// <see cref="ObjectDisposedException"/>.
    /// </para>
    /// <para>
    /// Always dispose via <c>using</c>; fonts that are loaded but never
    /// registered will otherwise leak their native allocation until GC
    /// finalizes the SafeHandle.
    /// </para>
    /// </remarks>
    /// <example>
    /// <code>
    /// using var font = EmbeddedFont.FromFile("DejaVuSans.ttf");
    /// using var builder = DocumentBuilder.Create()
    ///     .RegisterEmbeddedFont("DejaVu", font);  // consumes `font`
    /// // ... font is now owned by `builder`; do not use the `font` variable again
    /// </code>
    /// </example>
    public sealed class EmbeddedFont : IDisposable
    {
        private IntPtr _handle;
        private bool _disposed;

        /// <summary>
        /// Loads a font from a file on disk. The PostScript name baked into
        /// the font file is used when the font is registered without an
        /// override.
        /// </summary>
        /// <exception cref="ArgumentNullException"><paramref name="path"/> is null.</exception>
        /// <exception cref="FileNotFoundException">The file does not exist.</exception>
        /// <exception cref="PdfException">The font file cannot be parsed.</exception>
        public static EmbeddedFont FromFile(string path)
        {
            ArgumentNullException.ThrowIfNull(path);
            if (!File.Exists(path))
                throw new FileNotFoundException("Font file not found", path);

            var handle = NativeMethods.PdfEmbeddedFontFromFile(path, out var errorCode);
            if (handle == IntPtr.Zero)
                ExceptionMapper.ThrowIfError(errorCode);
            return new EmbeddedFont(handle);
        }

        /// <summary>
        /// Loads a font from a byte buffer. Pass <paramref name="name"/> to
        /// override the PostScript name recorded in the PDF.
        /// </summary>
        /// <exception cref="ArgumentNullException"><paramref name="data"/> is null.</exception>
        /// <exception cref="PdfException">The buffer is not a valid TTF/OTF file.</exception>
        public static EmbeddedFont FromBytes(byte[] data, string? name = null)
        {
            ArgumentNullException.ThrowIfNull(data);
            if (data.Length == 0)
                throw new ArgumentException("data is empty", nameof(data));

            var handle = NativeMethods.PdfEmbeddedFontFromBytes(data, (nuint)data.Length, name, out var errorCode);
            if (handle == IntPtr.Zero)
                ExceptionMapper.ThrowIfError(errorCode);
            return new EmbeddedFont(handle);
        }

        private EmbeddedFont(IntPtr handle)
        {
            _handle = handle;
        }

        /// <summary>
        /// The raw native handle. Internal — used by
        /// <see cref="DocumentBuilder.RegisterEmbeddedFont(string, EmbeddedFont)"/>
        /// to transfer ownership.
        /// </summary>
        internal IntPtr Handle
        {
            get
            {
                ObjectDisposedException.ThrowIf(_disposed, this);
                return _handle;
            }
        }

        /// <summary>
        /// Mark this handle as consumed by another object (e.g., a
        /// <see cref="DocumentBuilder"/> taking ownership on registration).
        /// No native free is called; the new owner takes responsibility.
        /// </summary>
        internal void MarkConsumed()
        {
            _handle = IntPtr.Zero;
            _disposed = true;
        }

        /// <summary>
        /// Release the native font handle if it hasn't been consumed by a
        /// <see cref="DocumentBuilder"/>. Safe to call multiple times.
        /// </summary>
        public void Dispose()
        {
            if (!_disposed && _handle != IntPtr.Zero)
            {
                NativeMethods.PdfEmbeddedFontFree(_handle);
                _handle = IntPtr.Zero;
            }
            _disposed = true;
        }
    }
}
