using System;
using PdfOxide.Exceptions;
using PdfOxide.Internal;

namespace PdfOxide.Core
{
    /// <summary>
    /// 1D / 2D barcode format supported by {@link Barcode.Generate}.
    /// Values mirror the integer codes the Rust FFI expects in
    /// <c>pdf_generate_barcode</c> (see <c>src/ffi.rs:1974</c>).
    /// </summary>
    public enum BarcodeFormat
    {
        /// <summary>Code-128 (alphanumeric, the default).</summary>
        Code128 = 0,
        /// <summary>Code-39 (upper-case alphanumeric).</summary>
        Code39 = 1,
        /// <summary>EAN-13 (retail product code, 13 digits).</summary>
        Ean13 = 2,
        /// <summary>EAN-8 (short retail code, 8 digits).</summary>
        Ean8 = 3,
        /// <summary>UPC-A (US retail, 12 digits).</summary>
        UpcA = 4,
        /// <summary>Interleaved 2-of-5 (numeric industrial).</summary>
        Itf = 5,
    }

    /// <summary>
    /// A generated 1D/2D barcode image. Wraps the FFI `FfiBarcodeImage`
    /// handle — call {@link Dispose} (or use `using`) to free it.
    /// </summary>
    public sealed class Barcode : IDisposable
    {
        private IntPtr _handle;
        private bool _disposed;

        private Barcode(IntPtr handle)
        {
            _handle = handle;
        }

        /// <summary>Generate a QR code image from the given payload.</summary>
        /// <param name="data">Payload to encode (text, URL, etc.).</param>
        /// <param name="errorCorrection">0=Low, 1=Medium, 2=Quartile, 3=High (default Medium).</param>
        /// <param name="sizePx">Target size in pixels (clamped to ≥1).</param>
        public static Barcode GenerateQrCode(string data, int errorCorrection = 1, int sizePx = 300)
        {
            ArgumentNullException.ThrowIfNull(data);
            if (data.Length == 0) throw new ArgumentException("data must not be empty.", nameof(data));
            if (sizePx <= 0) throw new ArgumentException("sizePx must be > 0.", nameof(sizePx));

            var handle = NativeMethods.PdfGenerateQrCode(data, errorCorrection, sizePx, out int err);
            if (handle == IntPtr.Zero)
            {
                ExceptionMapper.ThrowIfError(err);
            }
            return new Barcode(handle);
        }

        /// <summary>
        /// Generate a barcode image from the given payload.
        /// </summary>
        /// <param name="data">Payload to encode (alphabet depends on <paramref name="format"/>).</param>
        /// <param name="format">Barcode format / symbology.</param>
        /// <param name="sizePx">Target width in pixels (clamped to ≥1).</param>
        public static Barcode Generate(string data, BarcodeFormat format = BarcodeFormat.Code128, int sizePx = 300)
        {
            ArgumentNullException.ThrowIfNull(data);
            if (data.Length == 0) throw new ArgumentException("data must not be empty.", nameof(data));
            if (sizePx <= 0) throw new ArgumentException("sizePx must be > 0.", nameof(sizePx));

            var handle = NativeMethods.PdfGenerateBarcode(data, (int)format, sizePx, out int err);
            if (handle == IntPtr.Zero)
            {
                ExceptionMapper.ThrowIfError(err);
            }
            return new Barcode(handle);
        }

        /// <summary>The barcode format that was requested at generation time.</summary>
        public BarcodeFormat Format
        {
            get
            {
                ThrowIfDisposed();
                int v = NativeMethods.PdfBarcodeGetFormat(_handle, out int err);
                ExceptionMapper.ThrowIfError(err);
                return (BarcodeFormat)v;
            }
        }

        /// <summary>The source payload encoded in the barcode.</summary>
        public string Data
        {
            get
            {
                ThrowIfDisposed();
                var ptr = NativeMethods.PdfBarcodeGetData(_handle, out int err);
                ExceptionMapper.ThrowIfError(err);
                if (ptr == IntPtr.Zero) return string.Empty;
                try { return StringMarshaler.PtrToString(ptr); }
                finally { NativeMethods.FreeString(ptr); }
            }
        }

        /// <summary>
        /// Confidence score, 0.0..=1.0. Generated barcodes always return
        /// 1.0 since they're constructed exactly — the field exists to
        /// keep the surface symmetric with barcode detection APIs.
        /// </summary>
        public float Confidence
        {
            get
            {
                ThrowIfDisposed();
                float v = NativeMethods.PdfBarcodeGetConfidence(_handle, out int err);
                ExceptionMapper.ThrowIfError(err);
                return v;
            }
        }

        /// <summary>
        /// Raw PNG bytes of the generated barcode image.
        /// </summary>
        /// <param name="sizePx">
        /// Advisory target width; the Rust side currently ignores this
        /// parameter and returns the buffer produced at
        /// <see cref="Generate"/> time. Kept in the signature so future
        /// resampling is ABI-compatible.
        /// </param>
        public byte[] ToPng(int sizePx = 300)
        {
            ThrowIfDisposed();
            var ptr = NativeMethods.PdfBarcodeGetImagePng(_handle, sizePx, out int dataLen, out int err);
            ExceptionMapper.ThrowIfError(err);
            if (ptr == IntPtr.Zero || dataLen <= 0) return Array.Empty<byte>();
            try
            {
                var bytes = new byte[dataLen];
                System.Runtime.InteropServices.Marshal.Copy(ptr, bytes, 0, dataLen);
                return bytes;
            }
            finally
            {
                NativeMethods.FreeBytes(ptr);
            }
        }

        /// <summary>Returns the barcode as a vector SVG string.</summary>
        public string ToSvg()
        {
            ThrowIfDisposed();
            var ptr = NativeMethods.PdfBarcodeGetSvg(_handle, 0, out int err);
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
                    NativeMethods.PdfBarcodeFree(_handle);
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
