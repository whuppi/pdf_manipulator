using System;
using PdfOxide.Exceptions;
using PdfOxide.Internal;

namespace PdfOxide.Core
{
    /// <summary>
    /// OCR engine backed by PaddleOCR ONNX models.
    ///
    /// The native library must be built with the <c>ocr</c> feature on
    /// for this type to succeed — otherwise every method surfaces
    /// <see cref="UnsupportedFeatureException"/>.
    /// </summary>
    /// <remarks>
    /// Engines are reusable: load once, call
    /// <see cref="ExtractText(PdfDocument, int)"/> for each page.
    /// Dispose to release native model memory.
    /// </remarks>
    public sealed class OcrEngine : IDisposable
    {
        private IntPtr _handle;
        private bool _disposed;

        private OcrEngine(IntPtr handle)
        {
            _handle = handle;
        }

        /// <summary>
        /// Load detection + recognition ONNX models from disk.
        /// </summary>
        /// <param name="detectionModelPath">Path to the detection .onnx file.</param>
        /// <param name="recognitionModelPath">Path to the recognition .onnx file.</param>
        /// <param name="dictionaryPath">Path to the character dictionary (ppocr_keys_v1.txt).</param>
        public static OcrEngine Load(string detectionModelPath, string recognitionModelPath, string dictionaryPath)
        {
            ArgumentNullException.ThrowIfNull(detectionModelPath);
            ArgumentNullException.ThrowIfNull(recognitionModelPath);
            ArgumentNullException.ThrowIfNull(dictionaryPath);

            var handle = NativeMethods.pdf_ocr_engine_create(
                detectionModelPath,
                recognitionModelPath,
                dictionaryPath,
                out int err);
            if (handle == IntPtr.Zero)
            {
                ExceptionMapper.ThrowIfError(err);
                throw new PdfException("pdf_ocr_engine_create returned null with no error code");
            }
            return new OcrEngine(handle);
        }

        /// <summary>
        /// Classify a page as scanned-text-needs-OCR vs already-has-text.
        /// Does not require an <see cref="OcrEngine"/> instance.
        /// </summary>
        public static bool PageNeedsOcr(PdfDocument document, int pageIndex)
        {
            ArgumentNullException.ThrowIfNull(document);
            // NativeHandle variant: wrap the IntPtr through the document's
            // internal NativeHandle by calling the NativeHandle-typed
            // P/Invoke directly through a lightweight scope.
            var needsOcr = NativeMethods.OcrPageNeedsOcrByPtr(document.Handle, pageIndex, out int err);
            ExceptionMapper.ThrowIfError(err);
            return needsOcr;
        }

        /// <summary>
        /// Run OCR on one page and return the recognised text.
        /// </summary>
        public string ExtractText(PdfDocument document, int pageIndex)
        {
            ArgumentNullException.ThrowIfNull(document);
            ThrowIfDisposed();
            var ptr = NativeMethods.OcrExtractTextByPtr(document.Handle, pageIndex, _handle, out int err);
            ExceptionMapper.ThrowIfError(err);
            if (ptr == IntPtr.Zero) return string.Empty;
            try { return StringMarshaler.PtrToString(ptr); }
            finally { NativeMethods.FreeString(ptr); }
        }

        /// <inheritdoc />
        public void Dispose()
        {
            if (!_disposed)
            {
                if (_handle != IntPtr.Zero)
                {
                    NativeMethods.pdf_ocr_engine_free(_handle);
                    _handle = IntPtr.Zero;
                }
                _disposed = true;
            }
        }

        private void ThrowIfDisposed()
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
        }
    }
}
