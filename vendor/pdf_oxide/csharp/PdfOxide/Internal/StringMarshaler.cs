using System;
using System.Runtime.InteropServices;

namespace PdfOxide.Internal
{
    /// <summary>
    /// Utility for marshaling strings between C# and Rust.
    /// </summary>
    internal static class StringMarshaler
    {
        /// <summary>
        /// Converts a native UTF-8 pointer to a managed string.
        /// </summary>
        /// <param name="ptr">Pointer to UTF-8 null-terminated string.</param>
        /// <returns>The managed string, or empty string if pointer is null.</returns>
        public static string PtrToString(IntPtr ptr)
        {
            if (ptr == IntPtr.Zero)
                return string.Empty;

            return Marshal.PtrToStringUTF8(ptr) ?? string.Empty;
        }

        /// <summary>
        /// Converts a native UTF-8 pointer to a managed string and frees the memory.
        /// </summary>
        /// <param name="ptr">Pointer to UTF-8 null-terminated string.</param>
        /// <returns>The managed string, or empty string if pointer is null.</returns>
        public static string PtrToStringAndFree(IntPtr ptr)
        {
            try
            {
                return PtrToString(ptr);
            }
            finally
            {
                if (ptr != IntPtr.Zero)
                {
                    NativeMethods.FreeString(ptr);
                }
            }
        }
    }
}
