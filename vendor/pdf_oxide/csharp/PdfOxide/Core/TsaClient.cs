using System;
using System.Runtime.InteropServices;
using PdfOxide.Exceptions;
using PdfOxide.Internal;

namespace PdfOxide.Core
{
    /// <summary>
    /// Options for <see cref="TsaClient"/>. URL is required; everything
    /// else has sensible defaults matching the Rust-core config
    /// (SHA-256, nonce on, cert-req on, 30-second timeout).
    /// </summary>
    public sealed class TsaClientOptions
    {
        /// <summary>TSA endpoint URL, e.g. <c>https://freetsa.org/tsr</c>.</summary>
        public required string Url { get; init; }

        /// <summary>Optional HTTP Basic-auth username.</summary>
        public string? Username { get; init; }

        /// <summary>Optional HTTP Basic-auth password.</summary>
        public string? Password { get; init; }

        /// <summary>Request timeout in seconds. 0 or negative falls back to 30s.</summary>
        public int TimeoutSeconds { get; init; } = 30;

        /// <summary>Message-imprint hash algorithm.</summary>
        public TimestampHashAlgorithm HashAlgorithm { get; init; } = TimestampHashAlgorithm.Sha256;

        /// <summary>
        /// When true (default), include a random nonce to prevent
        /// response replay. Some TSAs disallow nonces; flip to false
        /// for those.
        /// </summary>
        public bool UseNonce { get; init; } = true;

        /// <summary>Whether to ask the TSA to include its certificate in the response.</summary>
        public bool CertReq { get; init; } = true;
    }

    /// <summary>
    /// RFC 3161 Time Stamp Authority client. <see cref="RequestTimestamp"/>
    /// hashes the input bytes and POSTs the request; the returned
    /// <see cref="Timestamp"/> is ready for inspection via the same
    /// accessors that parse a PDF-embedded timestamp.
    /// </summary>
    public sealed class TsaClient : IDisposable
    {
        private IntPtr _handle;
        private bool _disposed;

        private TsaClient(IntPtr handle)
        {
            _handle = handle;
        }

        /// <summary>Create a TSA client from <paramref name="options"/>.</summary>
        /// <exception cref="ArgumentNullException"><paramref name="options"/> is null.</exception>
        /// <exception cref="PdfException">The Rust core couldn't build the client.</exception>
        public static TsaClient Create(TsaClientOptions options)
        {
            ArgumentNullException.ThrowIfNull(options);

            var handle = NativeMethods.pdf_tsa_client_create(
                options.Url,
                options.Username ?? string.Empty,
                options.Password ?? string.Empty,
                options.TimeoutSeconds,
                (int)options.HashAlgorithm,
                options.UseNonce,
                options.CertReq,
                out int err);
            if (handle == IntPtr.Zero)
            {
                ExceptionMapper.ThrowIfError(err);
                throw new PdfException("pdf_tsa_client_create returned null with no error code");
            }
            return new TsaClient(handle);
        }

        /// <summary>
        /// Hash <paramref name="data"/> with the configured algorithm and
        /// request a timestamp for the digest. Network operation — throws
        /// on transport failure or if the TSA rejects the request.
        /// </summary>
        public Timestamp RequestTimestamp(byte[] data)
        {
            ThrowIfDisposed();
            ArgumentNullException.ThrowIfNull(data);

            var pinned = GCHandle.Alloc(data, GCHandleType.Pinned);
            try
            {
                var tsHandle = NativeMethods.pdf_tsa_request_timestamp(
                    _handle, pinned.AddrOfPinnedObject(), (UIntPtr)data.Length, out int err);
                if (tsHandle == IntPtr.Zero)
                {
                    ExceptionMapper.ThrowIfError(err);
                    throw new PdfException("pdf_tsa_request_timestamp returned null with no error code");
                }
                return Timestamp.FromRawHandle(tsHandle);
            }
            finally
            {
                pinned.Free();
            }
        }

        /// <summary>
        /// Request a timestamp for a pre-computed <paramref name="hash"/>.
        /// <paramref name="hashAlgorithm"/> must describe what produced
        /// the bytes.
        /// </summary>
        public Timestamp RequestTimestampHash(byte[] hash, TimestampHashAlgorithm hashAlgorithm)
        {
            ThrowIfDisposed();
            ArgumentNullException.ThrowIfNull(hash);

            var pinned = GCHandle.Alloc(hash, GCHandleType.Pinned);
            try
            {
                var tsHandle = NativeMethods.pdf_tsa_request_timestamp_hash(
                    _handle, pinned.AddrOfPinnedObject(), (UIntPtr)hash.Length,
                    (int)hashAlgorithm, out int err);
                if (tsHandle == IntPtr.Zero)
                {
                    ExceptionMapper.ThrowIfError(err);
                    throw new PdfException("pdf_tsa_request_timestamp_hash returned null with no error code");
                }
                return Timestamp.FromRawHandle(tsHandle);
            }
            finally
            {
                pinned.Free();
            }
        }

        /// <inheritdoc />
        public void Dispose()
        {
            if (!_disposed && _handle != IntPtr.Zero)
            {
                NativeMethods.pdf_tsa_client_free(_handle);
                _handle = IntPtr.Zero;
                _disposed = true;
            }
        }

        private void ThrowIfDisposed()
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
        }
    }
}
