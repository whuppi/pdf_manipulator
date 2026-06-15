using System;
using PdfOxide.Exceptions;
using PdfOxide.Internal;

namespace PdfOxide.Core
{
    /// <summary>
    /// High-level fluent builder for programmatic multi-page PDF construction.
    /// Wraps the Rust <c>DocumentBuilder</c> via the C FFI and exposes the
    /// full Phase 1 / Phase 2 / Phase 3 write-side API (metadata, embedded
    /// fonts, page content, annotations, HTML+CSS, AES-256 encryption).
    /// </summary>
    /// <remarks>
    /// <para>
    /// The fluent chain is single-use: terminal methods (<see cref="Build"/>,
    /// <see cref="Save(string)"/>, <see cref="SaveEncrypted"/>,
    /// <see cref="ToBytesEncrypted"/>) CONSUME the builder — subsequent
    /// calls throw <see cref="ObjectDisposedException"/>. Always wrap in a
    /// <c>using</c> so an exception mid-chain doesn't leak the handle.
    /// </para>
    /// <para>
    /// Only one <see cref="PageBuilder"/> may be open per builder at a
    /// time. Calling a second <see cref="A4Page"/> / <see cref="LetterPage"/> /
    /// <see cref="Page(float, float)"/> before the prior page's
    /// <see cref="PageBuilder.Done"/> throws <see cref="PdfException"/>.
    /// </para>
    /// </remarks>
    /// <example>
    /// <code>
    /// using var font = EmbeddedFont.FromFile("DejaVuSans.ttf");
    /// using var builder = DocumentBuilder.Create()
    ///     .Title("Hello")
    ///     .RegisterEmbeddedFont("DejaVu", font);
    /// var page = builder.A4Page()
    ///     .Font("DejaVu", 12)
    ///     .At(72, 720).Text("Привет, мир!")
    ///     .At(72, 700).Text("Καλημέρα κόσμε");
    /// page.Done();
    /// byte[] pdf = builder.Build();
    /// </code>
    /// </example>
    public sealed class DocumentBuilder : IDisposable
    {
        private IntPtr _handle;
        private bool _consumed;
        private PageBuilder? _openPage;

        private DocumentBuilder(IntPtr handle)
        {
            _handle = handle;
        }

        /// <summary>Create a fresh empty builder.</summary>
        public static DocumentBuilder Create()
        {
            var handle = NativeMethods.PdfDocumentBuilderCreate(out var errorCode);
            if (handle == IntPtr.Zero)
                ExceptionMapper.ThrowIfError(errorCode);
            return new DocumentBuilder(handle);
        }

        internal IntPtr Handle
        {
            get
            {
                ObjectDisposedException.ThrowIf(_consumed, this);
                return _handle;
            }
        }

        internal void ClearOpenPage()
        {
            _openPage = null;
        }

        private void CheckNoOpenPage()
        {
            if (_openPage != null)
                throw new PdfException("A PageBuilder is already open; call PageBuilder.Done() first.");
        }

        // --- Metadata -------------------------------------------------------

        /// <summary>Set the document title.</summary>
        public DocumentBuilder Title(string title)
        {
            ArgumentNullException.ThrowIfNull(title);
            CheckNoOpenPage();
            NativeMethods.PdfDocumentBuilderSetTitle(Handle, title, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Set the document author.</summary>
        public DocumentBuilder Author(string author)
        {
            ArgumentNullException.ThrowIfNull(author);
            CheckNoOpenPage();
            NativeMethods.PdfDocumentBuilderSetAuthor(Handle, author, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Set the document subject.</summary>
        public DocumentBuilder Subject(string subject)
        {
            ArgumentNullException.ThrowIfNull(subject);
            CheckNoOpenPage();
            NativeMethods.PdfDocumentBuilderSetSubject(Handle, subject, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Set the document keywords (comma-separated).</summary>
        public DocumentBuilder Keywords(string keywords)
        {
            ArgumentNullException.ThrowIfNull(keywords);
            CheckNoOpenPage();
            NativeMethods.PdfDocumentBuilderSetKeywords(Handle, keywords, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Set the creator application name.</summary>
        public DocumentBuilder Creator(string creator)
        {
            ArgumentNullException.ThrowIfNull(creator);
            CheckNoOpenPage();
            NativeMethods.PdfDocumentBuilderSetCreator(Handle, creator, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Run JavaScript when the document is opened (/OpenAction).</summary>
        public DocumentBuilder OnOpen(string script)
        {
            ArgumentNullException.ThrowIfNull(script);
            CheckNoOpenPage();
            NativeMethods.PdfDocumentBuilderOnOpen(Handle, script, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Enable PDF/UA-1 tagged PDF mode.
        /// Emits /MarkInfo, /StructTreeRoot, /Lang, and /ViewerPreferences
        /// in the catalog. Opt-in — no effect unless called. Bundle F-1/F-2.
        /// </summary>
        public DocumentBuilder TaggedPdfUa1()
        {
            NativeMethods.PdfDocumentBuilderTaggedPdfUa1(Handle, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Set the document's natural language tag, e.g. "en-US".
        /// Emitted as /Lang in the catalog when TaggedPdfUa1() is set. Bundle F-2.
        /// </summary>
        public DocumentBuilder Language(string lang)
        {
            ArgumentNullException.ThrowIfNull(lang);
            NativeMethods.PdfDocumentBuilderLanguage(Handle, lang, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Add a role-map entry: custom structure type → standard PDF structure type.
        /// Emitted in /RoleMap inside the StructTreeRoot when TaggedPdfUa1() is set.
        /// Multiple calls accumulate entries. Bundle F-4.
        /// </summary>
        public DocumentBuilder RoleMap(string custom, string standard)
        {
            ArgumentNullException.ThrowIfNull(custom);
            ArgumentNullException.ThrowIfNull(standard);
            NativeMethods.PdfDocumentBuilderRoleMap(Handle, custom, standard, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Register a TTF/OTF font under <paramref name="name"/>. The font
        /// is CONSUMED on success; do not use it after this call. On error
        /// the font handle is still valid and can be disposed normally.
        /// </summary>
        public DocumentBuilder RegisterEmbeddedFont(string name, EmbeddedFont font)
        {
            ArgumentNullException.ThrowIfNull(name);
            ArgumentNullException.ThrowIfNull(font);
            CheckNoOpenPage();
            var rc = NativeMethods.PdfDocumentBuilderRegisterEmbeddedFont(
                Handle, name, font.Handle, out var ec);
            if (rc != 0)
            {
                ExceptionMapper.ThrowIfError(ec);
                throw new PdfException("Native font registration failed without providing an error code.");
            }
            font.MarkConsumed();  // FFI took ownership on success
            return this;
        }

        // --- Page opening ---------------------------------------------------

        /// <summary>Start a new A4 page.</summary>
        public PageBuilder A4Page()
        {
            CheckNoOpenPage();
            var ptr = NativeMethods.PdfDocumentBuilderA4Page(Handle, out var ec);
            if (ptr == IntPtr.Zero)
                ExceptionMapper.ThrowIfError(ec);
            _openPage = new PageBuilder(this, ptr);
            return _openPage;
        }

        /// <summary>Start a new US Letter page.</summary>
        public PageBuilder LetterPage()
        {
            CheckNoOpenPage();
            var ptr = NativeMethods.PdfDocumentBuilderLetterPage(Handle, out var ec);
            if (ptr == IntPtr.Zero)
                ExceptionMapper.ThrowIfError(ec);
            _openPage = new PageBuilder(this, ptr);
            return _openPage;
        }

        /// <summary>Start a page with custom dimensions in PDF points (72 pt = 1 inch).</summary>
        public PageBuilder Page(float width, float height)
        {
            CheckNoOpenPage();
            var ptr = NativeMethods.PdfDocumentBuilderPage(Handle, width, height, out var ec);
            if (ptr == IntPtr.Zero)
                ExceptionMapper.ThrowIfError(ec);
            _openPage = new PageBuilder(this, ptr);
            return _openPage;
        }

        // --- Finalisation ---------------------------------------------------

        private IntPtr ConsumeHandle()
        {
            ObjectDisposedException.ThrowIf(_consumed, this);
            CheckNoOpenPage();
            var h = _handle;
            _consumed = true;
            return h;
        }

        /// <summary>Build the PDF and return it as bytes. CONSUMES the builder.</summary>
        public byte[] Build()
        {
            var h = ConsumeHandle();
            var ptr = NativeMethods.PdfDocumentBuilderBuild(h, out var outLen, out var ec);
            if (ptr == IntPtr.Zero)
            {
                NativeMethods.PdfDocumentBuilderFree(h);
                _handle = IntPtr.Zero;
                ExceptionMapper.ThrowIfError(ec);
                throw new PdfException("PDF build failed: native builder returned a null buffer.");
            }
            if (outLen > int.MaxValue)
            {
                NativeMethods.FreeBytes(ptr);
                NativeMethods.PdfDocumentBuilderFree(h);
                _handle = IntPtr.Zero;
                throw new OverflowException("Built PDF exceeds the maximum supported managed byte[] size.");
            }
            var outLenInt = (int)outLen;
            try
            {
                var bytes = new byte[outLenInt];
                System.Runtime.InteropServices.Marshal.Copy(ptr, bytes, 0, outLenInt);
                return bytes;
            }
            finally
            {
                NativeMethods.FreeBytes(ptr);
                // FFI consumed the inner builder, but the wrapper Box is still alive.
                NativeMethods.PdfDocumentBuilderFree(h);
                _handle = IntPtr.Zero;
            }
        }

        /// <summary>Save the PDF to <paramref name="path"/>. CONSUMES the builder.</summary>
        public void Save(string path)
        {
            ArgumentNullException.ThrowIfNull(path);
            var h = ConsumeHandle();
            try
            {
                var rc = NativeMethods.PdfDocumentBuilderSave(h, path, out var ec);
                if (rc != 0)
                    ExceptionMapper.ThrowIfError(ec);
            }
            finally
            {
                NativeMethods.PdfDocumentBuilderFree(h);
                _handle = IntPtr.Zero;
            }
        }

        /// <summary>Save the PDF with AES-256 encryption. CONSUMES the builder.</summary>
        public void SaveEncrypted(string path, string userPassword, string ownerPassword)
        {
            ArgumentNullException.ThrowIfNull(path);
            ArgumentNullException.ThrowIfNull(userPassword);
            ArgumentNullException.ThrowIfNull(ownerPassword);
            var h = ConsumeHandle();
            try
            {
                var rc = NativeMethods.PdfDocumentBuilderSaveEncrypted(
                    h, path, userPassword, ownerPassword, out var ec);
                if (rc != 0)
                    ExceptionMapper.ThrowIfError(ec);
            }
            finally
            {
                NativeMethods.PdfDocumentBuilderFree(h);
                _handle = IntPtr.Zero;
            }
        }

        /// <summary>Return the PDF as encrypted bytes (AES-256). CONSUMES the builder.</summary>
        public byte[] ToBytesEncrypted(string userPassword, string ownerPassword)
        {
            ArgumentNullException.ThrowIfNull(userPassword);
            ArgumentNullException.ThrowIfNull(ownerPassword);
            var h = ConsumeHandle();
            var ptr = NativeMethods.PdfDocumentBuilderToBytesEncrypted(
                h, userPassword, ownerPassword, out var outLen, out var ec);
            if (ptr == IntPtr.Zero)
            {
                NativeMethods.PdfDocumentBuilderFree(h);
                _handle = IntPtr.Zero;
                ExceptionMapper.ThrowIfError(ec);
                throw new PdfException("Encrypted PDF build failed: native builder returned a null buffer.");
            }
            if (outLen > int.MaxValue)
            {
                NativeMethods.FreeBytes(ptr);
                NativeMethods.PdfDocumentBuilderFree(h);
                _handle = IntPtr.Zero;
                throw new OverflowException("Built PDF exceeds the maximum supported managed byte[] size.");
            }
            var outLenInt = (int)outLen;
            try
            {
                var bytes = new byte[outLenInt];
                System.Runtime.InteropServices.Marshal.Copy(ptr, bytes, 0, outLenInt);
                return bytes;
            }
            finally
            {
                NativeMethods.FreeBytes(ptr);
                NativeMethods.PdfDocumentBuilderFree(h);
                _handle = IntPtr.Zero;
            }
        }

        /// <summary>Release native resources if the builder wasn't consumed.</summary>
        public void Dispose()
        {
            if (!_consumed && _handle != IntPtr.Zero)
            {
                NativeMethods.PdfDocumentBuilderFree(_handle);
                _handle = IntPtr.Zero;
                _consumed = true;
            }
        }
    }
}
