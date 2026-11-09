using System;
using System.Runtime.InteropServices;
using PdfOxide.Exceptions;
using PdfOxide.Internal;

namespace PdfOxide.Core
{
    /// <summary>
    /// The hash algorithm a timestamp's message imprint was computed
    /// with (RFC 3161 <c>MessageImprint.hashAlgorithm</c>). Numeric
    /// values match the FFI contract pinned in
    /// <c>signatures::timestamp::HashAlgorithm</c>.
    /// </summary>
    public enum TimestampHashAlgorithm
    {
        /// <summary>Any algorithm the Rust core couldn't classify.</summary>
        Unknown = 0,
        /// <summary>SHA-1 (legacy).</summary>
        Sha1 = 1,
        /// <summary>SHA-256 (modern default).</summary>
        Sha256 = 2,
        /// <summary>SHA-384.</summary>
        Sha384 = 3,
        /// <summary>SHA-512.</summary>
        Sha512 = 4,
    }

    /// <summary>
    /// An RFC 3161 timestamp — parsed from a DER TimeStampToken (the
    /// CMS-wrapped response from a TSA) or a bare TSTInfo. The
    /// inspection surface (issuer, serial, accuracy, digests) is
    /// implemented; <see cref="Verify"/> surfaces as
    /// <see cref="UnsupportedFeatureException"/> until the Rust CMS
    /// signature-verification path lands.
    /// </summary>
    public sealed class Timestamp : IDisposable
    {
        private IntPtr _handle;
        private bool _disposed;

        private Timestamp(IntPtr handle)
        {
            _handle = handle;
        }

        /// <summary>
        /// Internal factory used by <see cref="TsaClient"/> to hand off
        /// a freshly-allocated FFI handle without re-parsing bytes.
        /// </summary>
        internal static Timestamp FromRawHandle(IntPtr handle) => new(handle);

        /// <summary>
        /// Parse a DER-encoded RFC 3161 TimeStampToken (or bare
        /// TSTInfo) into a Timestamp.
        /// </summary>
        /// <exception cref="ArgumentNullException"><paramref name="data"/> is null.</exception>
        /// <exception cref="ArgumentException"><paramref name="data"/> is empty.</exception>
        /// <exception cref="PdfException">The bytes don't parse as a TimeStampToken or TSTInfo.</exception>
        public static Timestamp Parse(byte[] data)
        {
            ArgumentNullException.ThrowIfNull(data);
            if (data.Length == 0)
                throw new ArgumentException("Timestamp data must not be empty.", nameof(data));

            var pinned = GCHandle.Alloc(data, GCHandleType.Pinned);
            try
            {
                var handle = NativeMethods.pdf_timestamp_parse(
                    pinned.AddrOfPinnedObject(), (UIntPtr)data.Length, out int err);
                if (handle == IntPtr.Zero)
                {
                    ExceptionMapper.ThrowIfError(err);
                    throw new PdfException("pdf_timestamp_parse returned null with no error code");
                }
                return new Timestamp(handle);
            }
            finally
            {
                pinned.Free();
            }
        }

        /// <summary>Generation time from the TSTInfo (<c>genTime</c>).</summary>
        public DateTimeOffset Time
        {
            get
            {
                ThrowIfDisposed();
                var epoch = NativeMethods.pdf_timestamp_get_time(_handle, out int err);
                ExceptionMapper.ThrowIfError(err);
                return DateTimeOffset.FromUnixTimeSeconds(epoch);
            }
        }

        /// <summary>Serial number as a hex string (no <c>0x</c> prefix).</summary>
        public string Serial => ReadString(NativeMethods.pdf_timestamp_get_serial);

        /// <summary>TSA policy OID in dotted-decimal form.</summary>
        public string PolicyOid => ReadString(NativeMethods.pdf_timestamp_get_policy_oid);

        /// <summary>
        /// Name of the Time-Stamp Authority, as declared in the
        /// TSTInfo <c>tsa</c> GeneralName, or empty string if not
        /// included.
        /// </summary>
        public string TsaName => ReadString(NativeMethods.pdf_timestamp_get_tsa_name);

        /// <summary>Hash algorithm of the message imprint.</summary>
        public TimestampHashAlgorithm HashAlgorithm
        {
            get
            {
                ThrowIfDisposed();
                int code = NativeMethods.pdf_timestamp_get_hash_algorithm(_handle, out int err);
                ExceptionMapper.ThrowIfError(err);
                return (TimestampHashAlgorithm)code;
            }
        }

        /// <summary>Raw message-imprint hash bytes.</summary>
        public byte[] MessageImprint
        {
            get
            {
                ThrowIfDisposed();
                // Native returns a BORROWED pointer into the Timestamp's
                // own buffer (see src/ffi.rs:3589-3608); do NOT free.
                var ptr = NativeMethods.pdf_timestamp_get_message_imprint(
                    _handle, out UIntPtr len, out int err);
                ExceptionMapper.ThrowIfError(err);
                if (ptr == IntPtr.Zero || len == UIntPtr.Zero) return Array.Empty<byte>();
                ulong nativeLen = len.ToUInt64();
                if (nativeLen > int.MaxValue)
                    throw new OverflowException("Message-imprint length exceeds managed array limits.");
                var bytes = new byte[(int)nativeLen];
                Marshal.Copy(ptr, bytes, 0, bytes.Length);
                return bytes;
            }
        }

        /// <summary>
        /// Cryptographically verify the timestamp's signer. Currently
        /// unsupported — the RFC 3161 TSA-token signer-verification
        /// path is not yet wired through the FFI.
        /// </summary>
        /// <exception cref="UnsupportedFeatureException">Always, until Rust-core TSA-token verify lands.</exception>
        public bool Verify()
        {
            ThrowIfDisposed();
            var ok = NativeMethods.pdf_timestamp_verify(_handle, out int err);
            ExceptionMapper.ThrowIfError(err);
            return ok;
        }

        /// <inheritdoc />
        public void Dispose()
        {
            if (!_disposed && _handle != IntPtr.Zero)
            {
                NativeMethods.pdf_timestamp_free(_handle);
                _handle = IntPtr.Zero;
                _disposed = true;
            }
        }

        private delegate IntPtr NativeStringAccessor(IntPtr handle, out int errorCode);

        private string ReadString(NativeStringAccessor accessor)
        {
            ThrowIfDisposed();
            var ptr = accessor(_handle, out int err);
            ExceptionMapper.ThrowIfError(err);
            if (ptr == IntPtr.Zero) return string.Empty;
            try { return StringMarshaler.PtrToString(ptr); }
            finally { NativeMethods.FreeString(ptr); }
        }

        private void ThrowIfDisposed()
        {
            ObjectDisposedException.ThrowIf(_disposed, this);
        }
    }
}
