using PdfOxide.Exceptions;

namespace PdfOxide.Internal
{
    /// <summary>
    /// Maps native error codes to .NET exceptions.
    /// </summary>
    internal static class ExceptionMapper
    {
        /// <summary>
        /// Creates an exception from a non-zero error code. Code 0 is success and is invalid input.
        /// </summary>
        /// <param name="errorCode">The error code from the Rust FFI layer. Must not be 0.</param>
        /// <returns>An appropriate <see cref="PdfOxide.Exceptions.PdfException"/> subclass.</returns>
        /// <exception cref="System.ArgumentOutOfRangeException">Thrown when <paramref name="errorCode"/> is 0.</exception>
        public static PdfOxide.Exceptions.PdfException CreateException(int errorCode)
        {
            if (errorCode == 0)
            {
                throw new System.ArgumentOutOfRangeException(nameof(errorCode), "Cannot create an exception from success code 0.");
            }
            // Codes must match src/ffi.rs:48-56 exactly.
            // Prior versions of this table were offset by one — FFI code 8
            // (unsupported) was labelled SignatureException, causing the
            // u/gevorgter Reddit regression where a render failure surfaced
            // as a misleading signature error on Windows 11.
            return errorCode switch
            {
                1 => new InvalidParameter(
                    "Invalid argument: one or more arguments were invalid"),
                2 => new IoException(
                    "I/O error: file not found, permission denied, or read/write failed"),
                3 => new ParseException(
                    "Parse error: invalid PDF structure or content stream"),
                4 => new ParseException(
                    "Extraction failed: page content could not be extracted"),
                5 => new InternalError(
                    "Internal error: unexpected failure in the core library"),
                6 => new InvalidParameter(
                    "Invalid page index: page out of range for this document"),
                7 => new SearchException(
                    "Search error: search operation failed"),
                8 => new UnsupportedFeatureException(
                    "Unsupported feature: this build was compiled without support for the requested operation"),
                _ => new UnknownError($"Unknown error (code: {errorCode})")
            };
        }

        /// <summary>
        /// Checks if an error code represents success.
        /// </summary>
        /// <param name="errorCode">The error code.</param>
        /// <returns>True if the error code indicates success (0), false otherwise.</returns>
        public static bool IsSuccess(int errorCode) => errorCode == 0;

        /// <summary>
        /// Throws an exception if the error code indicates an error.
        /// </summary>
        /// <param name="errorCode">The error code to check.</param>
        /// <exception cref="PdfOxide.Exceptions.PdfException">Thrown if the error code indicates an error.</exception>
        public static void ThrowIfError(int errorCode)
        {
            if (!IsSuccess(errorCode))
            {
                throw CreateException(errorCode);
            }
        }
    }
}
