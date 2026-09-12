using System;
using System.Collections.Generic;
using PdfOxide.Exceptions;
using PdfOxide.Internal;

namespace PdfOxide.Core
{
    /// <summary>
    /// PDF/A conformance level, mapping to Rust's <c>PdfALevel</c>.
    /// See <c>src/ffi.rs:4699</c> for the integer encoding.
    /// </summary>
    public enum PdfALevel
    {
        /// <summary>PDF/A-1b — visual reproducibility baseline.</summary>
        A1b = 0,
        /// <summary>PDF/A-1a — accessible (tagged) variant of 1b.</summary>
        A1a = 1,
        /// <summary>PDF/A-2b — visual reproducibility with JPEG2000 + layers.</summary>
        A2b = 2,
        /// <summary>PDF/A-2a — accessible variant of 2b.</summary>
        A2a = 3,
        /// <summary>PDF/A-2u — Unicode-mapped variant of 2b.</summary>
        A2u = 4,
        /// <summary>PDF/A-3b — PDF/A-2b plus arbitrary embedded files.</summary>
        A3b = 5,
        /// <summary>PDF/A-3a — accessible variant of 3b.</summary>
        A3a = 6,
        /// <summary>PDF/A-3u — Unicode-mapped variant of 3b.</summary>
        A3u = 7,
    }

    /// <summary>PDF/X conformance level.</summary>
    public enum PdfXLevel
    {
        /// <summary>PDF/X-1a (print-ready CMYK).</summary>
        X1a = 0,
        /// <summary>PDF/X-3 (colour-managed).</summary>
        X3 = 1,
        /// <summary>PDF/X-4 (transparency allowed).</summary>
        X4 = 2,
    }

    /// <summary>PDF/UA accessibility conformance level.</summary>
    public enum PdfUaLevel
    {
        /// <summary>PDF/UA-1 (ISO 14289-1:2014).</summary>
        Ua1 = 1,
        /// <summary>PDF/UA-2 (ISO 14289-2).</summary>
        Ua2 = 2,
    }

    /// <summary>
    /// Result of a PDF/A, PDF/X, or PDF/UA validation pass.
    /// </summary>
    public sealed class PdfValidationResult
    {
        /// <summary>True when zero errors (warnings tolerated).</summary>
        public bool IsCompliant { get; }

        /// <summary>Human-readable error messages.</summary>
        public IReadOnlyList<string> Errors { get; }

        /// <summary>Human-readable warnings (non-blocking).</summary>
        public IReadOnlyList<string> Warnings { get; }

        internal PdfValidationResult(bool isCompliant, List<string> errors, List<string> warnings)
        {
            IsCompliant = isCompliant;
            Errors = errors;
            Warnings = warnings;
        }
    }

    /// <summary>
    /// Static helpers that run Rust-core PDF/A, PDF/X, and PDF/UA
    /// validation against an opened <see cref="PdfDocument"/>.
    /// </summary>
    public static class PdfValidator
    {
        /// <summary>
        /// Validate <paramref name="document"/> against a PDF/A level.
        /// </summary>
        public static PdfValidationResult ValidatePdfA(PdfDocument document, PdfALevel level = PdfALevel.A2b)
        {
            ArgumentNullException.ThrowIfNull(document);
            var docPtr = document.Handle;
            var results = NativeMethods.PdfValidatePdfALevel(docPtr, (int)level, out int err);
            if (results == IntPtr.Zero)
            {
                ExceptionMapper.ThrowIfError(err);
                throw new PdfException("PDF/A validation returned a null handle");
            }
            try
            {
                bool compliant = NativeMethods.PdfPdfAIsCompliant(results, out int complianceErr);
                ExceptionMapper.ThrowIfError(complianceErr);
                int errCount = NativeMethods.PdfPdfAErrorCount(results);
                // PDF/A warning count is exposed but no text accessor
                // — include in Errors via the error list only.
                var errors = ReadStrings(results, errCount, NativeMethods.PdfPdfAGetError);
                return new PdfValidationResult(compliant, errors, new List<string>());
            }
            finally
            {
                NativeMethods.PdfPdfAResultsFree(results);
            }
        }

        /// <summary>
        /// Validate <paramref name="document"/> against a PDF/X level.
        /// </summary>
        public static PdfValidationResult ValidatePdfX(PdfDocument document, PdfXLevel level = PdfXLevel.X4)
        {
            ArgumentNullException.ThrowIfNull(document);
            var docPtr = document.Handle;
            var results = NativeMethods.PdfValidatePdfXLevel(docPtr, (int)level, out int err);
            if (results == IntPtr.Zero)
            {
                ExceptionMapper.ThrowIfError(err);
                throw new PdfException("PDF/X validation returned a null handle");
            }
            try
            {
                bool compliant = NativeMethods.PdfPdfXIsCompliant(results, out int complianceErr);
                ExceptionMapper.ThrowIfError(complianceErr);
                int errCount = NativeMethods.PdfPdfXErrorCount(results);
                var errors = ReadStrings(results, errCount, NativeMethods.PdfPdfXGetError);
                return new PdfValidationResult(compliant, errors, new List<string>());
            }
            finally
            {
                NativeMethods.PdfPdfXResultsFree(results);
            }
        }

        /// <summary>
        /// Validate <paramref name="document"/> against a PDF/UA level.
        /// </summary>
        public static PdfValidationResult ValidatePdfUA(PdfDocument document, PdfUaLevel level = PdfUaLevel.Ua1)
        {
            ArgumentNullException.ThrowIfNull(document);
            var docPtr = document.Handle;
            var results = NativeMethods.PdfValidatePdfUa(docPtr, (int)level, out int err);
            if (results == IntPtr.Zero)
            {
                ExceptionMapper.ThrowIfError(err);
                throw new PdfException("PDF/UA validation returned a null handle");
            }
            try
            {
                bool compliant = NativeMethods.PdfPdfUaIsAccessible(results, out int complianceErr);
                ExceptionMapper.ThrowIfError(complianceErr);
                int errCount = NativeMethods.PdfPdfUaErrorCount(results);
                int warnCount = NativeMethods.PdfPdfUaWarningCount(results);
                var errors = ReadStrings(results, errCount, NativeMethods.PdfPdfUaGetError);
                var warnings = ReadStrings(results, warnCount, NativeMethods.PdfPdfUaGetWarning);
                return new PdfValidationResult(compliant, errors, warnings);
            }
            finally
            {
                NativeMethods.PdfPdfUaResultsFree(results);
            }
        }

        private delegate IntPtr GetStringAt(IntPtr results, int index, out int errorCode);

        private static List<string> ReadStrings(IntPtr results, int count, GetStringAt accessor)
        {
            var list = new List<string>(Math.Max(count, 0));
            for (int i = 0; i < count; i++)
            {
                var ptr = accessor(results, i, out int err);
                if (err != 0) { continue; }
                if (ptr == IntPtr.Zero) { continue; }
                try { list.Add(StringMarshaler.PtrToString(ptr)); }
                finally { NativeMethods.FreeString(ptr); }
            }
            return list;
        }
    }
}
