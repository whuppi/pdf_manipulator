using System;
using System.IO;
using System.Threading;
using System.Threading.Tasks;
using PdfOxide.Exceptions;
using PdfOxide.Internal;

namespace PdfOxide.Core
{
    /// <summary>
    /// Represents a PDF document that can be created, edited, and saved.
    /// Universal API combining creation, reading, and editing capabilities.
    /// </summary>
    /// <remarks>
    /// <para>
    /// Pdf is the universal PDF API that provides:
    /// <list type="bullet">
    /// <item><description>Creating PDFs from Markdown, HTML, or plain text</description></item>
    /// <item><description>Saving to file or memory buffer</description></item>
    /// <item><description>Editing page content and metadata</description></item>
    /// <item><description>Extracting content and converting formats</description></item>
    /// </list>
    /// </para>
    /// <para>
    /// The document must be explicitly disposed to release native resources.
    /// Use 'using' statements for automatic cleanup.
    /// </para>
    /// </remarks>
    /// <example>
    /// <code>
    /// // Create PDF from Markdown
    /// using (var pdf = Pdf.FromMarkdown("# Hello\n\n**Bold** text"))
    /// {
    ///     pdf.Save("output.pdf");
    /// }
    ///
    /// // Create from HTML
    /// using (var pdf = Pdf.FromHtml("<h1>Title</h1><p>Content</p>"))
    /// {
    ///     byte[] bytes = pdf.SaveToBytes();
    ///     File.WriteAllBytes("output.pdf", bytes);
    /// }
    /// </code>
    /// </example>
    public sealed class Pdf : IDisposable
    {
        private NativeHandle _handle;
        private bool _disposed;

        private Pdf(NativeHandle handle)
        {
            _handle = handle ?? throw new ArgumentNullException(nameof(handle));
        }

        /// <summary>
        /// Creates a PDF from Markdown content.
        /// </summary>
        /// <param name="markdown">The Markdown content.</param>
        /// <returns>A new Pdf document.</returns>
        /// <exception cref="ArgumentNullException">Thrown if <paramref name="markdown"/> is null.</exception>
        /// <exception cref="PdfException">Thrown if PDF creation fails.</exception>
        /// <example>
        /// <code>
        /// using (var pdf = Pdf.FromMarkdown("# Title\n\nParagraph text"))
        /// {
        ///     pdf.Save("document.pdf");
        /// }
        /// </code>
        /// </example>
        public static Pdf FromMarkdown(string markdown)
        {
            ArgumentNullException.ThrowIfNull(markdown);

            var handle = NativeMethods.PdfFromMarkdown(markdown, out var errorCode);
            if (handle.IsInvalid)
            {
                ExceptionMapper.ThrowIfError(errorCode);
            }

            return new Pdf(handle);
        }

        /// <summary>
        /// Creates a PDF from HTML content.
        /// </summary>
        /// <param name="html">The HTML content.</param>
        /// <returns>A new Pdf document.</returns>
        /// <exception cref="ArgumentNullException">Thrown if <paramref name="html"/> is null.</exception>
        /// <exception cref="PdfException">Thrown if PDF creation fails.</exception>
        /// <example>
        /// <code>
        /// using (var pdf = Pdf.FromHtml("<h1>Title</h1><p>Content</p>"))
        /// {
        ///     pdf.Save("document.pdf");
        /// }
        /// </code>
        /// </example>
        public static Pdf FromHtml(string html)
        {
            ArgumentNullException.ThrowIfNull(html);

            var handle = NativeMethods.PdfFromHtml(html, out var errorCode);
            if (handle.IsInvalid)
            {
                ExceptionMapper.ThrowIfError(errorCode);
            }

            return new Pdf(handle);
        }

        /// <summary>
        /// Creates a PDF by rendering HTML + CSS with a single embedded
        /// font. The font must cover every codepoint used by
        /// <paramref name="html"/>, or unknown glyphs fall back to
        /// <c>.notdef</c>.
        /// </summary>
        /// <param name="html">The HTML content.</param>
        /// <param name="css">The CSS stylesheet applied to the HTML.</param>
        /// <param name="fontBytes">TTF/OTF font bytes used for the body text.</param>
        /// <returns>A new <see cref="Pdf"/> document.</returns>
        /// <exception cref="ArgumentNullException">Any argument is null.</exception>
        /// <exception cref="PdfException">Rendering fails.</exception>
        /// <example>
        /// <code>
        /// byte[] font = File.ReadAllBytes("DejaVuSans.ttf");
        /// using var pdf = Pdf.FromHtmlCss(
        ///     "&lt;h1&gt;Hello&lt;/h1&gt;&lt;p&gt;World&lt;/p&gt;",
        ///     "h1 { color: blue }",
        ///     font);
        /// pdf.Save("out.pdf");
        /// </code>
        /// </example>
        public static Pdf FromHtmlCss(string html, string css, byte[] fontBytes)
        {
            ArgumentNullException.ThrowIfNull(html);
            ArgumentNullException.ThrowIfNull(css);
            ArgumentNullException.ThrowIfNull(fontBytes);
            if (fontBytes.Length == 0) throw new ArgumentException("fontBytes is empty", nameof(fontBytes));

            var ptr = NativeMethods.PdfFromHtmlCss(html, css, fontBytes, (nuint)fontBytes.Length, out var errorCode);
            if (ptr == IntPtr.Zero)
                ExceptionMapper.ThrowIfError(errorCode);
            return new Pdf(new NativeHandle(ptr, p => NativeMethods.PdfFree(p)));
        }

        /// <summary>
        /// Creates a PDF from HTML+CSS with a multi-font cascade. The first
        /// entry of <paramref name="fonts"/> is the default used when a CSS
        /// <c>font-family</c> doesn't match any registered family.
        /// </summary>
        public static unsafe Pdf FromHtmlCssWithFonts(string html, string css,
            System.Collections.Generic.IReadOnlyList<System.Collections.Generic.KeyValuePair<string, byte[]>> fonts)
        {
            ArgumentNullException.ThrowIfNull(html);
            ArgumentNullException.ThrowIfNull(css);
            if (fonts == null || fonts.Count == 0)
                throw new ArgumentException("at least one font must be provided", nameof(fonts));

            int n = fonts.Count;
            // Pin every font's byte[] and every UTF-8-encoded name so the
            // native call sees stable pointers for the duration of the
            // FFI crossing.
            var byteHandles = new System.Runtime.InteropServices.GCHandle[n];
            var nameBuffers = new byte[n][];
            var nameHandles = new System.Runtime.InteropServices.GCHandle[n];
            var fontPointers = new IntPtr[n];
            var namePointers = new IntPtr[n];
            var fontLens = new nuint[n];
            try
            {
                for (int i = 0; i < n; i++)
                {
                    var kv = fonts[i];
                    if (kv.Value == null || kv.Value.Length == 0)
                        throw new ArgumentException($"fonts[{i}] has empty bytes", nameof(fonts));
                    byteHandles[i] = System.Runtime.InteropServices.GCHandle.Alloc(kv.Value, System.Runtime.InteropServices.GCHandleType.Pinned);
                    fontPointers[i] = byteHandles[i].AddrOfPinnedObject();
                    fontLens[i] = (nuint)kv.Value.Length;

                    // UTF-8 NUL-terminated name
                    var utf8 = System.Text.Encoding.UTF8.GetBytes(kv.Key + "\0");
                    nameBuffers[i] = utf8;
                    nameHandles[i] = System.Runtime.InteropServices.GCHandle.Alloc(utf8, System.Runtime.InteropServices.GCHandleType.Pinned);
                    namePointers[i] = nameHandles[i].AddrOfPinnedObject();
                }
                var fpHandle = System.Runtime.InteropServices.GCHandle.Alloc(fontPointers, System.Runtime.InteropServices.GCHandleType.Pinned);
                var npHandle = System.Runtime.InteropServices.GCHandle.Alloc(namePointers, System.Runtime.InteropServices.GCHandleType.Pinned);
                var flHandle = System.Runtime.InteropServices.GCHandle.Alloc(fontLens, System.Runtime.InteropServices.GCHandleType.Pinned);
                try
                {
                    var ptr = NativeMethods.PdfFromHtmlCssWithFonts(
                        html, css,
                        (IntPtr*)npHandle.AddrOfPinnedObject(),
                        (IntPtr*)fpHandle.AddrOfPinnedObject(),
                        (nuint*)flHandle.AddrOfPinnedObject(),
                        (nuint)n,
                        out var errorCode);
                    if (ptr == IntPtr.Zero)
                        ExceptionMapper.ThrowIfError(errorCode);
                    return new Pdf(new NativeHandle(ptr, p => NativeMethods.PdfFree(p)));
                }
                finally
                {
                    fpHandle.Free();
                    npHandle.Free();
                    flHandle.Free();
                }
            }
            finally
            {
                for (int i = 0; i < n; i++)
                {
                    if (byteHandles[i].IsAllocated) byteHandles[i].Free();
                    if (nameHandles[i].IsAllocated) nameHandles[i].Free();
                }
            }
        }

        /// <summary>
        /// Creates a PDF from plain text content.
        /// </summary>
        /// <param name="text">The text content.</param>
        /// <returns>A new Pdf document.</returns>
        /// <exception cref="ArgumentNullException">Thrown if <paramref name="text"/> is null.</exception>
        /// <exception cref="PdfException">Thrown if PDF creation fails.</exception>
        /// <example>
        /// <code>
        /// using (var pdf = Pdf.FromText("This is plain text"))
        /// {
        ///     pdf.Save("document.pdf");
        /// }
        /// </code>
        /// </example>
        public static Pdf FromText(string text)
        {
            ArgumentNullException.ThrowIfNull(text);

            var handle = NativeMethods.PdfFromText(text, out var errorCode);
            if (handle.IsInvalid)
            {
                ExceptionMapper.ThrowIfError(errorCode);
            }

            return new Pdf(handle);
        }

        /// <summary>
        /// Creates a single-page PDF wrapping a raster image on disk.
        /// Supported formats match the core <c>pdf_from_image</c> FFI
        /// entry point (JPEG, PNG).
        /// </summary>
        /// <param name="path">Path to the image file.</param>
        /// <returns>A new <see cref="Pdf"/>.</returns>
        /// <exception cref="ArgumentNullException">Thrown if <paramref name="path"/> is null.</exception>
        /// <exception cref="PdfException">Thrown if the image cannot be read or converted.</exception>
        public static Pdf FromImage(string path)
        {
            ArgumentNullException.ThrowIfNull(path);

            var handle = NativeMethods.PdfFromImage(path, out var errorCode);
            if (handle.IsInvalid)
            {
                ExceptionMapper.ThrowIfError(errorCode);
            }
            return new Pdf(handle);
        }

        /// <summary>
        /// Creates a single-page PDF wrapping a raster image already in
        /// memory (JPEG or PNG bytes). Use this overload when the image
        /// comes from a network response or a database blob and you want
        /// to avoid a scratch file.
        /// </summary>
        /// <param name="data">Raw image bytes.</param>
        /// <returns>A new <see cref="Pdf"/>.</returns>
        /// <exception cref="ArgumentNullException">Thrown if <paramref name="data"/> is null.</exception>
        /// <exception cref="ArgumentException">Thrown if <paramref name="data"/> is empty.</exception>
        /// <exception cref="PdfException">Thrown if the image is malformed or an unsupported format.</exception>
        public static Pdf FromImageBytes(byte[] data)
        {
            ArgumentNullException.ThrowIfNull(data);
            if (data.Length == 0)
                throw new ArgumentException("Image byte array must not be empty.", nameof(data));

            var handle = NativeMethods.PdfFromImageBytes(data, data.Length, out var errorCode);
            if (handle.IsInvalid)
            {
                ExceptionMapper.ThrowIfError(errorCode);
            }
            return new Pdf(handle);
        }

        /// <summary>
        /// Gets the number of pages in the PDF.
        /// </summary>
        /// <value>The page count.</value>
        /// <exception cref="ObjectDisposedException">Thrown if the document has been disposed.</exception>
        /// <exception cref="PdfException">Thrown if page count cannot be determined.</exception>
        public int PageCount
        {
            get
            {
                ThrowIfDisposed();
                var count = NativeMethods.PdfGetPageCount(_handle.DangerousGetHandle(), out var errorCode);
                ExceptionMapper.ThrowIfError(errorCode);
                return count;
            }
        }

        /// <summary>
        /// Saves the PDF to a file.
        /// </summary>
        /// <param name="path">The output file path.</param>
        /// <exception cref="ArgumentNullException">Thrown if <paramref name="path"/> is null.</exception>
        /// <exception cref="ObjectDisposedException">Thrown if the document has been disposed.</exception>
        /// <exception cref="PdfIoException">Thrown if the file cannot be written.</exception>
        /// <example>
        /// <code>
        /// using (var pdf = Pdf.FromMarkdown("# Hello"))
        /// {
        ///     pdf.Save("output.pdf");
        /// }
        /// </code>
        /// </example>
        public void Save(string path)
        {
            ArgumentNullException.ThrowIfNull(path);

            ThrowIfDisposed();

            var result = NativeMethods.PdfSave(_handle, path, out var errorCode);
            if (result != 0)
            {
                ExceptionMapper.ThrowIfError(errorCode);
            }
        }

        /// <summary>
        /// Saves the PDF to a byte array.
        /// </summary>
        /// <returns>The PDF content as bytes.</returns>
        /// <exception cref="ObjectDisposedException">Thrown if the document has been disposed.</exception>
        /// <exception cref="PdfException">Thrown if the PDF cannot be generated.</exception>
        /// <example>
        /// <code>
        /// using (var pdf = Pdf.FromMarkdown("# Hello"))
        /// {
        ///     byte[] pdfBytes = pdf.SaveToBytes();
        ///     File.WriteAllBytes("output.pdf", pdfBytes);
        /// }
        /// </code>
        /// </example>
        public byte[] SaveToBytes()
        {
            ThrowIfDisposed();

            var outputPtr = NativeMethods.PdfSaveToBytes(_handle, out var outputLen, out var errorCode);
            if (errorCode != 0)
            {
                ExceptionMapper.ThrowIfError(errorCode);
            }
            if (outputPtr == IntPtr.Zero || outputLen <= 0)
            {
                return System.Array.Empty<byte>();
            }

            try
            {
                var bytes = new byte[outputLen];
                System.Runtime.InteropServices.Marshal.Copy(outputPtr, bytes, 0, outputLen);
                return bytes;
            }
            finally
            {
                NativeMethods.FreeBytes(outputPtr);
            }
        }

        /// <summary>
        /// Saves the PDF to a stream.
        /// </summary>
        /// <param name="stream">The output stream.</param>
        /// <exception cref="ArgumentNullException">Thrown if <paramref name="stream"/> is null.</exception>
        /// <exception cref="ObjectDisposedException">Thrown if the document has been disposed.</exception>
        /// <exception cref="PdfException">Thrown if the PDF cannot be generated.</exception>
        /// <example>
        /// <code>
        /// using (var pdf = Pdf.FromMarkdown("# Hello"))
        /// using (var file = File.Create("output.pdf"))
        /// {
        ///     pdf.SaveToStream(file);
        /// }
        /// </code>
        /// </example>
        public void SaveToStream(Stream stream)
        {
            ArgumentNullException.ThrowIfNull(stream);

            byte[] bytes = SaveToBytes();
            stream.Write(bytes, 0, bytes.Length);
        }

        /// <summary>
        /// Asynchronously saves the PDF to a file.
        /// </summary>
        /// <param name="path">The output file path.</param>
        /// <param name="cancellationToken">A cancellation token.</param>
        /// <returns>A task that completes when the file is saved.</returns>
        /// <exception cref="ArgumentNullException">Thrown if <paramref name="path"/> is null.</exception>
        /// <exception cref="OperationCanceledException">Thrown if the operation is cancelled.</exception>
        public Task SaveAsync(string path, CancellationToken cancellationToken = default)
        {
            ArgumentNullException.ThrowIfNull(path);

            return Task.Run(() =>
            {
                cancellationToken.ThrowIfCancellationRequested();
                Save(path);
            }, cancellationToken);
        }

        /// <summary>
        /// Asynchronously saves the PDF to a stream.
        /// </summary>
        /// <param name="stream">The output stream.</param>
        /// <param name="cancellationToken">A cancellation token.</param>
        /// <returns>A task that completes when the PDF is saved.</returns>
        /// <exception cref="ArgumentNullException">Thrown if <paramref name="stream"/> is null.</exception>
        /// <exception cref="OperationCanceledException">Thrown if the operation is cancelled.</exception>
        public Task SaveToStreamAsync(Stream stream, CancellationToken cancellationToken = default)
        {
            ArgumentNullException.ThrowIfNull(stream);

            return Task.Run(() =>
            {
                cancellationToken.ThrowIfCancellationRequested();
                SaveToStream(stream);
            }, cancellationToken);
        }

        /// <summary>
        /// Disposes the PDF and releases native resources.
        /// </summary>
        public void Dispose()
        {
            if (!_disposed)
            {
                _handle?.Dispose();
                _disposed = true;
            }
        }

        private void ThrowIfDisposed()
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
        }
    }
}
