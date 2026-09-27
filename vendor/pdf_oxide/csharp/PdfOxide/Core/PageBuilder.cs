using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
using PdfOxide.Exceptions;
using PdfOxide.Internal;

namespace PdfOxide.Core
{
    /// <summary>
    /// Fluent per-page builder returned by <see cref="DocumentBuilder.A4Page"/>,
    /// <see cref="DocumentBuilder.LetterPage"/>, and
    /// <see cref="DocumentBuilder.Page(float, float)"/>.
    /// </summary>
    /// <remarks>
    /// All content methods return <c>this</c> for chaining. Call
    /// <see cref="Done"/> to commit the page back to its parent builder;
    /// after that the <see cref="PageBuilder"/> is invalid. Use
    /// <see cref="Dispose"/> only for error recovery when you want to
    /// drop the page without committing.
    /// </remarks>
    public sealed class PageBuilder : IDisposable
    {
        private readonly DocumentBuilder _parent;
        private IntPtr _handle;
        private bool _done;
        // Mirror of the last Font(name, size) set on this page and the
        // last At(y) cursor, used by the heuristic Measure /
        // RemainingSpace helpers. Kept in managed state because
        // v0.3.39 has no FFI for font-metric queries or cursor
        // readback (tracked as follow-up; see the Measure docstring).
        private string _lastFontName = "Helvetica";
        private float _lastFontSize = 12f;
        private float _lastCursorY = float.NaN;

        internal PageBuilder(DocumentBuilder parent, IntPtr handle)
        {
            _parent = parent;
            _handle = handle;
        }

        private IntPtr Handle
        {
            get
            {
                ObjectDisposedException.ThrowIf(_done, this);
                return _handle;
            }
        }

        /// <summary>Raw page handle — for use by StreamingTable within this assembly.</summary>
        internal IntPtr InternalHandle => Handle;

        // --- Content ops -----------------------------------------------------

        /// <summary>Set the font + size for subsequent text.</summary>
        public PageBuilder Font(string name, float size)
        {
            ArgumentNullException.ThrowIfNull(name);
            NativeMethods.PdfPageBuilderFont(Handle, name, size, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            _lastFontName = name;
            _lastFontSize = size;
            return this;
        }

        /// <summary>Move the cursor to absolute coordinates (points from lower-left).</summary>
        public PageBuilder At(float x, float y)
        {
            NativeMethods.PdfPageBuilderAt(Handle, x, y, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            _lastCursorY = y;
            return this;
        }

        /// <summary>Emit a line of text at the current cursor position.</summary>
        public PageBuilder Text(string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderText(Handle, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Emit a heading. <paramref name="level"/> is 1–6.</summary>
        public PageBuilder Heading(byte level, string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderHeading(Handle, level, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Emit a paragraph with automatic line wrapping.</summary>
        public PageBuilder Paragraph(string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderParagraph(Handle, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Advance the cursor by <paramref name="points"/>.</summary>
        public PageBuilder Space(float points)
        {
            NativeMethods.PdfPageBuilderSpace(Handle, points, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Draw a horizontal rule across the page.</summary>
        public PageBuilder HorizontalRule()
        {
            NativeMethods.PdfPageBuilderHorizontalRule(Handle, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        // --- Annotations (Phase 3) ------------------------------------------

        /// <summary>Attach a URL link to the previously-emitted text element.</summary>
        public PageBuilder LinkUrl(string url)
        {
            ArgumentNullException.ThrowIfNull(url);
            NativeMethods.PdfPageBuilderLinkUrl(Handle, url, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Link the previous text to an internal page (0-based).</summary>
        public PageBuilder LinkPage(int pageIndex)
        {
            NativeMethods.PdfPageBuilderLinkPage(Handle, (nuint)pageIndex, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Link the previous text to a named destination.</summary>
        public PageBuilder LinkNamed(string destination)
        {
            ArgumentNullException.ThrowIfNull(destination);
            NativeMethods.PdfPageBuilderLinkNamed(Handle, destination, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Link the previous text to a JavaScript action.</summary>
        public PageBuilder LinkJavascript(string script)
        {
            ArgumentNullException.ThrowIfNull(script);
            NativeMethods.PdfPageBuilderLinkJavascript(Handle, script, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Run JavaScript when this page is opened (/AA /O).</summary>
        public PageBuilder OnOpen(string script)
        {
            ArgumentNullException.ThrowIfNull(script);
            NativeMethods.PdfPageBuilderOnOpen(Handle, script, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Run JavaScript when this page is closed (/AA /C).</summary>
        public PageBuilder OnClose(string script)
        {
            ArgumentNullException.ThrowIfNull(script);
            NativeMethods.PdfPageBuilderOnClose(Handle, script, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Set a keystroke JS action (/AA /K) on the last form field.</summary>
        public PageBuilder FieldKeystroke(string script)
        {
            ArgumentNullException.ThrowIfNull(script);
            NativeMethods.PdfPageBuilderFieldKeystroke(Handle, script, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Set a format JS action (/AA /F) on the last form field.</summary>
        public PageBuilder FieldFormat(string script)
        {
            ArgumentNullException.ThrowIfNull(script);
            NativeMethods.PdfPageBuilderFieldFormat(Handle, script, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Set a validate JS action (/AA /V) on the last form field.</summary>
        public PageBuilder FieldValidate(string script)
        {
            ArgumentNullException.ThrowIfNull(script);
            NativeMethods.PdfPageBuilderFieldValidate(Handle, script, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Set a calculate JS action (/AA /C) on the last form field.</summary>
        public PageBuilder FieldCalculate(string script)
        {
            ArgumentNullException.ThrowIfNull(script);
            NativeMethods.PdfPageBuilderFieldCalculate(Handle, script, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Highlight the previous text with an RGB colour (0–1 channels).</summary>
        public PageBuilder Highlight(float r, float g, float b)
        {
            NativeMethods.PdfPageBuilderHighlight(Handle, r, g, b, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Underline the previous text.</summary>
        public PageBuilder Underline(float r, float g, float b)
        {
            NativeMethods.PdfPageBuilderUnderline(Handle, r, g, b, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Strike through the previous text.</summary>
        public PageBuilder Strikeout(float r, float g, float b)
        {
            NativeMethods.PdfPageBuilderStrikeout(Handle, r, g, b, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Squiggly-underline the previous text.</summary>
        public PageBuilder Squiggly(float r, float g, float b)
        {
            NativeMethods.PdfPageBuilderSquiggly(Handle, r, g, b, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Attach a sticky-note annotation to the previous text.</summary>
        public PageBuilder StickyNote(string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderStickyNote(Handle, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Place a sticky-note at an absolute position on the page.</summary>
        public PageBuilder StickyNoteAt(float x, float y, string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderStickyNoteAt(Handle, x, y, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Apply a text watermark to the page.</summary>
        public PageBuilder Watermark(string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderWatermark(Handle, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Apply the standard "CONFIDENTIAL" diagonal watermark.</summary>
        public PageBuilder WatermarkConfidential()
        {
            NativeMethods.PdfPageBuilderWatermarkConfidential(Handle, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Apply the standard "DRAFT" diagonal watermark.</summary>
        public PageBuilder WatermarkDraft()
        {
            NativeMethods.PdfPageBuilderWatermarkDraft(Handle, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Attach a standard stamp annotation at the cursor position
        /// (default 150×50 pt box). <paramref name="typeName"/> matches
        /// the PDF spec's standard stamps ("Approved", "NotApproved",
        /// "Draft", "Confidential", "Final", "Experimental", "Expired",
        /// "ForPublicRelease", "NotForPublicRelease", "AsIs", "Sold",
        /// "Departmental", "ForComment", "TopSecret"); any other name
        /// becomes a custom stamp.
        /// </summary>
        public PageBuilder Stamp(string typeName)
        {
            ArgumentNullException.ThrowIfNull(typeName);
            NativeMethods.PdfPageBuilderStamp(Handle, typeName, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Place a free-flowing text annotation inside the rectangle
        /// (x, y, w, h). Independent of the cursor flow.
        /// </summary>
        public PageBuilder FreeText(float x, float y, float w, float h, string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderFreetext(Handle, x, y, w, h, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        // --- Form-field widgets ---------------------------------------------

        /// <summary>
        /// Add a single-line text form field widget at the rectangle
        /// (x, y, w, h). <paramref name="name"/> is the unique field
        /// identifier used for form submission;
        /// <paramref name="defaultValue"/> is the initial text (pass
        /// null or empty for a blank field).
        /// </summary>
        public PageBuilder TextField(string name, float x, float y, float w, float h, string? defaultValue = null)
        {
            ArgumentNullException.ThrowIfNull(name);
            NativeMethods.PdfPageBuilderTextField(Handle, name, x, y, w, h, defaultValue, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Add a checkbox form field widget. <paramref name="checked"/>
        /// sets the initial state.
        /// </summary>
        public PageBuilder Checkbox(string name, float x, float y, float w, float h, bool @checked = false)
        {
            ArgumentNullException.ThrowIfNull(name);
            NativeMethods.PdfPageBuilderCheckbox(Handle, name, x, y, w, h, @checked, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Add a dropdown combo-box form field. Each entry of
        /// <paramref name="options"/> is a user-visible (and submitted)
        /// choice. <paramref name="selected"/> picks the initial value;
        /// pass null to leave blank.
        /// </summary>
        public unsafe PageBuilder ComboBox(string name, float x, float y, float w, float h,
            System.Collections.Generic.IReadOnlyList<string> options, string? selected = null)
        {
            ArgumentNullException.ThrowIfNull(name);
            if (options == null || options.Count == 0)
                throw new ArgumentException("options must be non-empty", nameof(options));
            int n = options.Count;
            var buffers = new byte[n][];
            var handles = new System.Runtime.InteropServices.GCHandle[n];
            var pointers = new IntPtr[n];
            try
            {
                for (int i = 0; i < n; i++)
                {
                    buffers[i] = System.Text.Encoding.UTF8.GetBytes(options[i] + "\0");
                    handles[i] = System.Runtime.InteropServices.GCHandle.Alloc(buffers[i], System.Runtime.InteropServices.GCHandleType.Pinned);
                    pointers[i] = handles[i].AddrOfPinnedObject();
                }
                var ptrHandle = System.Runtime.InteropServices.GCHandle.Alloc(pointers, System.Runtime.InteropServices.GCHandleType.Pinned);
                try
                {
                    NativeMethods.PdfPageBuilderComboBox(Handle, name, x, y, w, h,
                        (byte**)ptrHandle.AddrOfPinnedObject(), (nuint)n, selected, out var ec);
                    ExceptionMapper.ThrowIfError(ec);
                }
                finally { ptrHandle.Free(); }
            }
            finally
            {
                for (int i = 0; i < n; i++)
                    if (handles[i].IsAllocated) handles[i].Free();
            }
            return this;
        }

        /// <summary>
        /// Add a radio-button group. <paramref name="buttons"/> is a
        /// sequence of <c>(exportValue, x, y, w, h)</c> tuples — one
        /// entry per option. <paramref name="selected"/> picks the
        /// initial value.
        /// </summary>
        public unsafe PageBuilder RadioGroup(string name,
            System.Collections.Generic.IReadOnlyList<(string value, float x, float y, float w, float h)> buttons,
            string? selected = null)
        {
            ArgumentNullException.ThrowIfNull(name);
            if (buttons == null || buttons.Count == 0)
                throw new ArgumentException("buttons must be non-empty", nameof(buttons));
            int n = buttons.Count;
            var valueBuffers = new byte[n][];
            var handles = new System.Runtime.InteropServices.GCHandle[n];
            var pointers = new IntPtr[n];
            var xs = new float[n];
            var ys = new float[n];
            var ws = new float[n];
            var hs = new float[n];
            try
            {
                for (int i = 0; i < n; i++)
                {
                    valueBuffers[i] = System.Text.Encoding.UTF8.GetBytes(buttons[i].value + "\0");
                    handles[i] = System.Runtime.InteropServices.GCHandle.Alloc(valueBuffers[i], System.Runtime.InteropServices.GCHandleType.Pinned);
                    pointers[i] = handles[i].AddrOfPinnedObject();
                    xs[i] = buttons[i].x;
                    ys[i] = buttons[i].y;
                    ws[i] = buttons[i].w;
                    hs[i] = buttons[i].h;
                }
                var ptrHandle = System.Runtime.InteropServices.GCHandle.Alloc(pointers, System.Runtime.InteropServices.GCHandleType.Pinned);
                var xsH = System.Runtime.InteropServices.GCHandle.Alloc(xs, System.Runtime.InteropServices.GCHandleType.Pinned);
                var ysH = System.Runtime.InteropServices.GCHandle.Alloc(ys, System.Runtime.InteropServices.GCHandleType.Pinned);
                var wsH = System.Runtime.InteropServices.GCHandle.Alloc(ws, System.Runtime.InteropServices.GCHandleType.Pinned);
                var hsH = System.Runtime.InteropServices.GCHandle.Alloc(hs, System.Runtime.InteropServices.GCHandleType.Pinned);
                try
                {
                    NativeMethods.PdfPageBuilderRadioGroup(Handle, name,
                        (byte**)ptrHandle.AddrOfPinnedObject(),
                        (float*)xsH.AddrOfPinnedObject(),
                        (float*)ysH.AddrOfPinnedObject(),
                        (float*)wsH.AddrOfPinnedObject(),
                        (float*)hsH.AddrOfPinnedObject(),
                        (nuint)n, selected, out var ec);
                    ExceptionMapper.ThrowIfError(ec);
                }
                finally { ptrHandle.Free(); xsH.Free(); ysH.Free(); wsH.Free(); hsH.Free(); }
            }
            finally
            {
                for (int i = 0; i < n; i++)
                    if (handles[i].IsAllocated) handles[i].Free();
            }
            return this;
        }

        /// <summary>Add a clickable push button with a visible caption.</summary>
        public PageBuilder PushButton(string name, float x, float y, float w, float h, string caption)
        {
            ArgumentNullException.ThrowIfNull(name);
            ArgumentNullException.ThrowIfNull(caption);
            NativeMethods.PdfPageBuilderPushButton(Handle, name, x, y, w, h, caption, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Add an unsigned signature placeholder field (/FT /Sig).</summary>
        public PageBuilder SignatureField(string name, float x, float y, float w, float h)
        {
            ArgumentNullException.ThrowIfNull(name);
            NativeMethods.PdfPageBuilderSignatureField(Handle, name, x, y, w, h, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Add a footnote reference mark inline at the cursor and record
        /// <paramref name="noteText"/> for page-end placement with a separator line.
        /// </summary>
        public PageBuilder Footnote(string refMark, string noteText)
        {
            ArgumentNullException.ThrowIfNull(refMark);
            ArgumentNullException.ThrowIfNull(noteText);
            NativeMethods.PdfPageBuilderFootnote(Handle, refMark, noteText, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Lay out <paramref name="text"/> as balanced multi-column flow.
        /// Paragraphs in <paramref name="text"/> are separated by "\n\n".
        /// </summary>
        public PageBuilder Columns(uint columnCount, float gapPt, string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderColumns(Handle, columnCount, gapPt, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        // --- Rich text inline runs ------------------------------------------

        /// <summary>Emit <paramref name="text"/> inline (advances cursorX only).</summary>
        public PageBuilder Inline(string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderInline(Handle, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Inline bold run.</summary>
        public PageBuilder InlineBold(string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderInlineBold(Handle, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Inline italic run.</summary>
        public PageBuilder InlineItalic(string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderInlineItalic(Handle, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Inline colored run (RGB 0.0–1.0).</summary>
        public PageBuilder InlineColor(float r, float g, float b, string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderInlineColor(Handle, r, g, b, text, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Advance cursorY one line-height and reset cursorX to 72 pt.</summary>
        public PageBuilder Newline()
        {
            NativeMethods.PdfPageBuilderNewline(Handle, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        // --- Barcode / QR-code placement ------------------------------------

        /// <summary>
        /// Place a 1-D barcode image on the page.
        /// <paramref name="barcodeType"/>: 0=Code128 1=Code39 2=EAN13 3=EAN8
        /// 4=UPCA 5=ITF 6=Code93 7=Codabar.
        /// </summary>
        public PageBuilder Barcode1d(int barcodeType, string data, float x, float y, float w, float h)
        {
            ArgumentNullException.ThrowIfNull(data);
            NativeMethods.PdfPageBuilderBarcode1d(Handle, barcodeType, data, x, y, w, h, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Place a QR-code image on the page (square: <paramref name="size"/> × <paramref name="size"/> pt).</summary>
        public PageBuilder BarcodeQr(string data, float x, float y, float size)
        {
            ArgumentNullException.ThrowIfNull(data);
            NativeMethods.PdfPageBuilderBarcodeQr(Handle, data, x, y, size, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Embed a JPEG/PNG image at (x, y, w, h) in PDF points.
        /// No alt text or /Artifact wrapper is added — use
        /// <see cref="ImageWithAlt"/> or <see cref="ImageArtifact"/> for PDF/UA-1.
        /// </summary>
        public unsafe PageBuilder Image(byte[] imageBytes, float x, float y, float w, float h)
        {
            ArgumentNullException.ThrowIfNull(imageBytes);
            if (imageBytes.Length == 0) throw new ArgumentException("imageBytes must not be empty", nameof(imageBytes));
            fixed (byte* p = imageBytes)
            {
                NativeMethods.PdfPageBuilderImage(Handle, p, (nuint)imageBytes.Length, x, y, w, h, out var ec);
                ExceptionMapper.ThrowIfError(ec);
            }
            return this;
        }

        /// <summary>
        /// Embed an image with an accessibility alt text (PDF/UA-1 Figure element).
        /// <paramref name="imageBytes"/> must contain raw JPEG/PNG/WebP data.
        /// </summary>
        public unsafe PageBuilder ImageWithAlt(byte[] imageBytes, float x, float y, float w, float h, string altText)
        {
            ArgumentNullException.ThrowIfNull(imageBytes);
            ArgumentNullException.ThrowIfNull(altText);
            if (imageBytes.Length == 0) throw new ArgumentException("imageBytes must not be empty", nameof(imageBytes));
            fixed (byte* p = imageBytes)
            {
                NativeMethods.PdfPageBuilderImageWithAlt(Handle, p, (nuint)imageBytes.Length, x, y, w, h, altText, out var ec);
                ExceptionMapper.ThrowIfError(ec);
            }
            return this;
        }

        /// <summary>
        /// Embed a decorative image as an /Artifact (no alt text, PDF/UA-1).
        /// <paramref name="imageBytes"/> must contain raw JPEG/PNG/WebP data.
        /// </summary>
        public unsafe PageBuilder ImageArtifact(byte[] imageBytes, float x, float y, float w, float h)
        {
            ArgumentNullException.ThrowIfNull(imageBytes);
            if (imageBytes.Length == 0) throw new ArgumentException("imageBytes must not be empty", nameof(imageBytes));
            fixed (byte* p = imageBytes)
            {
                NativeMethods.PdfPageBuilderImageArtifact(Handle, p, (nuint)imageBytes.Length, x, y, w, h, out var ec);
                ExceptionMapper.ThrowIfError(ec);
            }
            return this;
        }

        // --- Low-level graphics primitives (PdfWriter exposure) -------------

        /// <summary>Draw a stroked rectangle outline (1pt black).</summary>
        public PageBuilder Rect(float x, float y, float w, float h)
        {
            NativeMethods.PdfPageBuilderRect(Handle, x, y, w, h, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Draw a filled rectangle in RGB colour (channels 0–1).</summary>
        public PageBuilder FilledRect(float x, float y, float w, float h, float r, float g, float b)
        {
            NativeMethods.PdfPageBuilderFilledRect(Handle, x, y, w, h, r, g, b, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>Draw a line from (x1, y1) to (x2, y2) with 1pt black stroke.</summary>
        public PageBuilder Line(float x1, float y1, float x2, float y2)
        {
            NativeMethods.PdfPageBuilderLine(Handle, x1, y1, x2, y2, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        // --- v0.3.39 primitives (#393) -------------------------------------

        /// <summary>
        /// Draw a stroked rectangle with caller-supplied line width
        /// and RGB colour (channels 0–1).
        /// </summary>
        public PageBuilder StrokeRect(float x, float y, float w, float h,
            float width = 1f, float r = 0f, float g = 0f, float b = 0f)
        {
            NativeMethods.PdfPageBuilderStrokeRect(Handle, x, y, w, h, width, r, g, b, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Draw a line from (x1, y1) to (x2, y2) with caller-supplied
        /// line width and RGB colour (channels 0–1).
        /// </summary>
        public PageBuilder StrokeLine(float x1, float y1, float x2, float y2,
            float width = 1f, float r = 0f, float g = 0f, float b = 0f)
        {
            NativeMethods.PdfPageBuilderStrokeLine(Handle, x1, y1, x2, y2, width, r, g, b, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Draw a dashed rectangle border. <paramref name="dash"/> contains alternating
        /// on/off lengths in points (e.g. [3, 2] = 3pt dash, 2pt gap).
        /// </summary>
        public unsafe PageBuilder StrokeRectDashed(float x, float y, float w, float h,
            float[] dash, float phase = 0f,
            float width = 1f, float r = 0f, float g = 0f, float b = 0f)
        {
            fixed (float* dp = dash)
            {
                NativeMethods.PdfPageBuilderStrokeRectDashed(Handle, x, y, w, h, width, r, g, b,
                    dp, (nuint)(dash?.Length ?? 0), phase, out var ec);
                ExceptionMapper.ThrowIfError(ec);
            }
            return this;
        }

        /// <summary>
        /// Draw a dashed line. <paramref name="dash"/> contains alternating
        /// on/off lengths in points (e.g. [5, 3] = 5pt dash, 3pt gap).
        /// </summary>
        public unsafe PageBuilder StrokeLineDashed(float x1, float y1, float x2, float y2,
            float[] dash, float phase = 0f,
            float width = 1f, float r = 0f, float g = 0f, float b = 0f)
        {
            fixed (float* dp = dash)
            {
                NativeMethods.PdfPageBuilderStrokeLineDashed(Handle, x1, y1, x2, y2, width, r, g, b,
                    dp, (nuint)(dash?.Length ?? 0), phase, out var ec);
                ExceptionMapper.ThrowIfError(ec);
            }
            return this;
        }

        /// <summary>
        /// Place word-wrapped text inside the rectangle (x, y, w, h)
        /// with the given horizontal alignment.
        /// </summary>
        /// <remarks>
        /// Text is flowed using the current font; overflow past the
        /// rectangle's lower edge is clipped (not auto-paginated —
        /// pair with <see cref="NewPageSameSize"/> if you need manual
        /// pagination).
        /// </remarks>
        public PageBuilder TextInRect(float x, float y, float w, float h,
            string text, Alignment align = Alignment.Left)
        {
            ArgumentNullException.ThrowIfNull(text);
            NativeMethods.PdfPageBuilderTextInRect(Handle, x, y, w, h, text, (int)align, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            return this;
        }

        /// <summary>
        /// Transition to a new page with the same dimensions and font
        /// configuration as the current one.
        /// </summary>
        /// <remarks>
        /// Cursor resets to the top-left margin. Any repeating header
        /// must be re-emitted explicitly by the caller.
        /// </remarks>
        public PageBuilder NewPageSameSize()
        {
            NativeMethods.PdfPageBuilderNewPageSameSize(Handle, out var ec);
            ExceptionMapper.ThrowIfError(ec);
            _lastCursorY = float.NaN;
            return this;
        }

        /// <summary>
        /// Approximate the width in points required to render
        /// <paramref name="text"/> at the current font + size.
        /// </summary>
        /// <remarks>
        /// <strong>v0.3.39 limitation:</strong> no font-metric FFI is
        /// available yet, so this returns a heuristic estimate using
        /// the current <see cref="Font(string, float)"/> size and an
        /// average-glyph-width factor of 0.5 (0.6 for monospaced
        /// font names). Accurate per-glyph measurement is tracked as
        /// a follow-up to #393. The signature is stable — consuming
        /// code will transparently get exact measurements once the
        /// FFI lands.
        /// </remarks>
        public float Measure(string text)
        {
            ArgumentNullException.ThrowIfNull(text);
            float factor = IsMonospace(_lastFontName) ? 0.6f : 0.5f;
            return text.Length * _lastFontSize * factor;
        }

        /// <summary>
        /// Approximate the vertical space remaining between the last
        /// cursor position set via <see cref="At(float, float)"/> and
        /// the bottom-of-page margin (72 pt default).
        /// </summary>
        /// <remarks>
        /// <strong>v0.3.39 limitation:</strong> the cursor value is
        /// mirrored in managed state from the most recent
        /// <see cref="At(float, float)"/> call — layout-advancing ops
        /// (<see cref="Text(string)"/>, <see cref="Paragraph(string)"/>,
        /// etc.) are NOT tracked. Returns <c>float.NaN</c> if the
        /// caller has not invoked <see cref="At(float, float)"/> yet.
        /// A precise FFI query is planned alongside the streaming
        /// Table surface (#393 step 6.5).
        /// </remarks>
        public float RemainingSpace()
        {
            if (float.IsNaN(_lastCursorY)) return float.NaN;
            const float bottomMargin = 72f;
            return _lastCursorY - bottomMargin;
        }

        private static bool IsMonospace(string fontName)
        {
            if (string.IsNullOrEmpty(fontName)) return false;
            return fontName.Contains("Courier", StringComparison.OrdinalIgnoreCase)
                || fontName.Contains("Mono", StringComparison.OrdinalIgnoreCase);
        }

        /// <summary>
        /// Place a buffered table at the current cursor position.
        /// Column widths, alignments, and cell contents are taken from
        /// <paramref name="spec"/>.
        /// </summary>
        /// <remarks>
        /// <para>
        /// Every row in <see cref="TableSpec.Rows"/> must have exactly
        /// <c>Columns.Count</c> cells. <see langword="null"/> cells
        /// render as empty strings.
        /// </para>
        /// <para>
        /// Memory is <c>O(rows * columns)</c>. Use
        /// <see cref="StreamingTable(System.Collections.Generic.IReadOnlyList{Column}, bool)"/>
        /// when you'd rather append rows one at a time; note that the
        /// streaming adapter still buffers internally in v0.3.39.
        /// </para>
        /// </remarks>
        public unsafe PageBuilder Table(TableSpec spec)
        {
            ArgumentNullException.ThrowIfNull(spec);
            ArgumentNullException.ThrowIfNull(spec.Columns);
            if (spec.Columns.Count == 0)
                throw new ArgumentException("spec.Columns must be non-empty", nameof(spec));
            if (spec.Rows == null)
                throw new ArgumentException("spec.Rows must not be null", nameof(spec));

            int nCols = spec.Columns.Count;
            int nRows = spec.Rows.Count;

            // Validate row widths up front so we don't leak half-encoded buffers.
            for (int r = 0; r < nRows; r++)
            {
                var row = spec.Rows[r];
                if (row == null || row.Count != nCols)
                    throw new ArgumentException(
                        $"row {r} has {row?.Count ?? 0} cells, expected {nCols}",
                        nameof(spec));
            }

            var widths = new float[nCols];
            var aligns = new int[nCols];
            for (int c = 0; c < nCols; c++)
            {
                widths[c] = spec.Columns[c].Width;
                aligns[c] = (int)spec.Columns[c].Align;
            }

            // If HasHeader, the first logical row in cell_strings is the
            // header — synthesise it from the column labels.
            int totalRows = spec.HasHeader ? nRows + 1 : nRows;
            int totalCells = totalRows * nCols;

            var cellBytes = new byte[totalCells][];
            int idx = 0;
            if (spec.HasHeader)
            {
                for (int c = 0; c < nCols; c++)
                    cellBytes[idx++] = Encoding.UTF8.GetBytes((spec.Columns[c].Header ?? string.Empty) + "\0");
            }
            for (int r = 0; r < nRows; r++)
            {
                var row = spec.Rows[r];
                for (int c = 0; c < nCols; c++)
                    cellBytes[idx++] = Encoding.UTF8.GetBytes((row[c] ?? string.Empty) + "\0");
            }

            var handles = new GCHandle[totalCells];
            var pointers = new IntPtr[totalCells];
            GCHandle ptrsH = default, widthsH = default, alignsH = default;
            try
            {
                for (int i = 0; i < totalCells; i++)
                {
                    handles[i] = GCHandle.Alloc(cellBytes[i], GCHandleType.Pinned);
                    pointers[i] = handles[i].AddrOfPinnedObject();
                }
                ptrsH = GCHandle.Alloc(pointers, GCHandleType.Pinned);
                widthsH = GCHandle.Alloc(widths, GCHandleType.Pinned);
                alignsH = GCHandle.Alloc(aligns, GCHandleType.Pinned);

                NativeMethods.PdfPageBuilderTable(
                    Handle,
                    (nuint)nCols,
                    (float*)widthsH.AddrOfPinnedObject(),
                    (int*)alignsH.AddrOfPinnedObject(),
                    (nuint)totalRows,
                    (byte**)ptrsH.AddrOfPinnedObject(),
                    spec.HasHeader ? 1 : 0,
                    out var ec);
                ExceptionMapper.ThrowIfError(ec);
            }
            finally
            {
                if (ptrsH.IsAllocated) ptrsH.Free();
                if (widthsH.IsAllocated) widthsH.Free();
                if (alignsH.IsAllocated) alignsH.Free();
                for (int i = 0; i < totalCells; i++)
                    if (handles[i].IsAllocated) handles[i].Free();
            }
            return this;
        }

        /// <summary>
        /// Open a streaming-table handle for the given columns. Rows
        /// are appended one at a time via
        /// <see cref="StreamingTable.AddRow(string[])"/> and finalised
        /// via <see cref="StreamingTable.Build"/>.
        /// </summary>
        /// <param name="columns">Column specifications.</param>
        /// <param name="repeatHeader">Repeat the header row on every page break.</param>
        /// <param name="mode">Column-sizing mode (default: <see cref="TableMode.Fixed"/>).</param>
        /// <param name="maxRowspan">
        /// Maximum rowspan value cells may carry. 0 or 1 disables rowspan (default).
        /// Pass ≥2 to enable <see cref="StreamingTable.AddRowSpan"/> for cells that
        /// should span multiple rows.
        /// </param>
        /// <param name="batchSize">
        /// Maximum rows to buffer before an automatic flush to the native layer.
        /// 0 or unset defaults to 256.
        /// </param>
        public StreamingTable StreamingTable(
            IReadOnlyList<Column> columns,
            bool repeatHeader = true,
            TableMode? mode = null,
            int maxRowspan = 1,
            int batchSize = 256)
        {
            ArgumentNullException.ThrowIfNull(columns);
            if (columns.Count == 0)
                throw new ArgumentException("columns must be non-empty", nameof(columns));
            return new StreamingTable(this, columns, repeatHeader, mode, maxRowspan, batchSize);
        }

        /// <summary>
        /// Commit the page's buffered operations back to the parent
        /// builder. Returns the parent for continued chaining. After
        /// <see cref="Done"/> this <see cref="PageBuilder"/> is invalid.
        /// </summary>
        public DocumentBuilder Done()
        {
            ObjectDisposedException.ThrowIf(_done, this);
            var rc = NativeMethods.PdfPageBuilderDone(_handle, out var ec);
            _done = true;
            _parent.ClearOpenPage();
            _handle = IntPtr.Zero;
            if (rc != 0)
                ExceptionMapper.ThrowIfError(ec);
            return _parent;
        }

        /// <summary>
        /// Drop the page without committing. Use for error recovery —
        /// the parent's open-page slot is released so the next
        /// <see cref="DocumentBuilder.A4Page"/> etc. succeeds.
        /// </summary>
        public void Dispose()
        {
            if (!_done && _handle != IntPtr.Zero)
            {
                NativeMethods.PdfPageBuilderFree(_handle);
                _parent.ClearOpenPage();
                _handle = IntPtr.Zero;
                _done = true;
            }
        }
    }
}
