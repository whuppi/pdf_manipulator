using System;
using System.Collections.Generic;

namespace PdfOxide.Exceptions
{
    /// <summary>
    /// Base sealed exception for all PDF Oxide errors, enabling exhaustive pattern matching.
    /// </summary>
    /// <remarks>
    /// <para>
    /// All PDF Oxide exceptions inherit from this sealed class, enabling pattern matching in C# 8+
    /// and providing a unified error handling framework across all language bindings.
    /// </para>
    /// <para>
    /// Error Code System (4-digit codes):
    /// <list type="bullet">
    ///     <item><description>1000-1999: Parse errors</description></item>
    ///     <item><description>2000-2999: I/O errors</description></item>
    ///     <item><description>3000-3999: Encryption errors</description></item>
    ///     <item><description>4000-4999: State errors</description></item>
    ///     <item><description>5000-5999: Unsupported feature errors</description></item>
    ///     <item><description>6000-6999: Validation errors</description></item>
    ///     <item><description>7000-7999: Rendering errors</description></item>
    ///     <item><description>8000-8999: Search errors</description></item>
    ///     <item><description>9000-9999: Other errors (compliance, OCR, etc.)</description></item>
    /// </list>
    /// </para>
    /// <example>
    /// <code>
    /// try
    /// {
    ///     document.RenderPage(0, outputPath, options);
    /// }
    /// catch (PdfException ex)
    /// {
    ///     var result = ex switch
    ///     {
    ///         ParseException => "PDF structure issue",
    ///         IoException => "File system issue",
    ///         EncryptionException => "Password required",
    ///         RenderingException => "Rendering failed",
    ///         _ => "Unknown error"
    ///     };
    ///     Console.WriteLine($"[{ex.Code}] {result}: {ex.Message}");
    /// }
    /// </code>
    /// </example>
    /// </remarks>
    public class PdfException : Exception
    {
        /// <summary>
        /// Gets the 4-digit error code (XXXX format).
        /// </summary>
        public string Code { get; }

        /// <summary>
        /// Gets additional error context (operation name, timestamp, parameters).
        /// </summary>
        public IReadOnlyDictionary<string, object?> Details { get; private set; }

        private Dictionary<string, object?> _mutableDetails;

        /// <summary>
        /// Initializes a new instance of the <see cref="PdfException"/> class with a simple message.
        /// Uses a default code of "9999" (other error).
        /// </summary>
        /// <param name="message">Human-readable error message</param>
        public PdfException(string message) : base(message)
        {
            Code = "9999";
            _mutableDetails = new Dictionary<string, object?>();
            Details = _mutableDetails;
        }

        /// <summary>
        /// Maps a low-level FFI error code (1-8) to a human-readable message.
        /// </summary>
        /// <param name="errorCode">Error code from the FFI layer.</param>
        /// <returns>A human-readable error message.</returns>
        public static string GetErrorMessage(int errorCode)
        {
            return errorCode switch
            {
                1 => "Invalid file path",
                2 => "Document not found or cannot be opened",
                3 => "Invalid PDF format",
                4 => "Text extraction failed",
                5 => "PDF parsing failed",
                6 => "Invalid page index",
                7 => "Search operation failed",
                8 => "Internal FFI error",
                _ => $"Unknown error code: {errorCode}"
            };
        }

        /// <summary>
        /// Initializes a new instance of the <see cref="PdfException"/> class.
        /// </summary>
        /// <param name="code">4-digit error code</param>
        /// <param name="message">Human-readable error message</param>
        /// <param name="details">Additional context information</param>
        public PdfException(
            string code,
            string message,
            Dictionary<string, object?>? details = null)
            : base(FormatMessage(code, message))
        {
            if (string.IsNullOrEmpty(code) || code.Length != 4 || !code.All(char.IsDigit))
            {
                throw new ArgumentException("Code must be 4 digits", nameof(code));
            }

            Code = code;
            _mutableDetails = details ?? new Dictionary<string, object?>();
            Details = _mutableDetails;
        }

        /// <summary>
        /// Initializes a new instance of the <see cref="PdfException"/> class with an inner exception.
        /// </summary>
        /// <param name="code">4-digit error code</param>
        /// <param name="message">Human-readable error message</param>
        /// <param name="innerException">The inner exception</param>
        /// <param name="details">Additional context information</param>
        public PdfException(
            string code,
            string message,
            Exception? innerException,
            Dictionary<string, object?>? details = null)
            : base(FormatMessage(code, message), innerException)
        {
            if (string.IsNullOrEmpty(code) || code.Length != 4 || !code.All(char.IsDigit))
            {
                throw new ArgumentException("Code must be 4 digits", nameof(code));
            }

            Code = code;
            _mutableDetails = details ?? new Dictionary<string, object?>();
            Details = _mutableDetails;
        }

        /// <summary>
        /// Adds operational context to this exception for better diagnostics.
        /// </summary>
        /// <param name="operation">Name of the operation that failed</param>
        /// <param name="context">Additional context key-value pairs</param>
        /// <returns>This exception for method chaining</returns>
        public PdfException WithContext(string operation, params (string key, object? value)[] context)
        {
            _mutableDetails["timestamp"] = DateTime.UtcNow.ToString("o");
            _mutableDetails["operation"] = operation;

            foreach (var (key, value) in context)
            {
                _mutableDetails[$"context_{key}"] = value;
            }

            return this;
        }

        private static string FormatMessage(string code, string message)
            => $"[{code}] {message}";
    }

    // ===== Parse Errors (1000-1999) =====

    /// <summary>
    /// Thrown when PDF structure parsing fails.
    /// </summary>
    public sealed class ParseException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="ParseException"/> class.
        /// </summary>
        public ParseException(string message, Dictionary<string, object?>? details = null)
            : base("1000", message, details) { }
    }

    /// <summary>
    /// Thrown when PDF has invalid structure.
    /// </summary>
    public sealed class InvalidStructure : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="InvalidStructure"/> class.
        /// </summary>
        public InvalidStructure(string message, Dictionary<string, object?>? details = null)
            : base("1101", message, details) { }
    }

    /// <summary>
    /// Thrown when PDF data is corrupted.
    /// </summary>
    public sealed class CorruptedData : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="CorruptedData"/> class.
        /// </summary>
        public CorruptedData(string message, Dictionary<string, object?>? details = null)
            : base("1200", message, details) { }
    }

    /// <summary>
    /// Thrown when PDF version is not supported.
    /// </summary>
    public sealed class UnsupportedVersion : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="UnsupportedVersion"/> class.
        /// </summary>
        public UnsupportedVersion(string message, Dictionary<string, object?>? details = null)
            : base("1300", message, details) { }
    }

    // ===== I/O Errors (2000-2999) =====

    /// <summary>
    /// Thrown when I/O operations fail.
    /// </summary>
    public sealed class IoException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="IoException"/> class.
        /// </summary>
        public IoException(string message, Dictionary<string, object?>? details = null)
            : base("2000", message, details) { }
    }

    /// <summary>
    /// Thrown when a required file is not found.
    /// </summary>
    public sealed class FileNotFound : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="FileNotFound"/> class.
        /// </summary>
        public FileNotFound(string message, Dictionary<string, object?>? details = null)
            : base("2100", message, details) { }
    }

    /// <summary>
    /// Thrown when file permissions prevent an operation.
    /// </summary>
    public sealed class PermissionDenied : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="PermissionDenied"/> class.
        /// </summary>
        public PermissionDenied(string message, Dictionary<string, object?>? details = null)
            : base("2200", message, details) { }
    }

    /// <summary>
    /// Thrown when there is insufficient disk space.
    /// </summary>
    public sealed class DiskFull : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="DiskFull"/> class.
        /// </summary>
        public DiskFull(string message, Dictionary<string, object?>? details = null)
            : base("2300", message, details) { }
    }

    /// <summary>
    /// Thrown when network operations fail.
    /// </summary>
    public sealed class NetworkError : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="NetworkError"/> class.
        /// </summary>
        public NetworkError(string message, Dictionary<string, object?>? details = null)
            : base("2400", message, details) { }
    }

    // ===== Encryption Errors (3000-3999) =====

    /// <summary>
    /// Thrown when encryption operations fail.
    /// </summary>
    public sealed class EncryptionException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="EncryptionException"/> class.
        /// </summary>
        public EncryptionException(string message, Dictionary<string, object?>? details = null)
            : base("3000", message, details) { }
    }

    /// <summary>
    /// Thrown when password is invalid.
    /// </summary>
    public sealed class InvalidPassword : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="InvalidPassword"/> class.
        /// </summary>
        public InvalidPassword(string message, Dictionary<string, object?>? details = null)
            : base("3100", message, details) { }
    }

    /// <summary>
    /// Thrown when decryption fails.
    /// </summary>
    public sealed class DecryptionFailed : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="DecryptionFailed"/> class.
        /// </summary>
        public DecryptionFailed(string message, Dictionary<string, object?>? details = null)
            : base("3200", message, details) { }
    }

    /// <summary>
    /// Thrown when encryption algorithm is not supported.
    /// </summary>
    public sealed class UnsupportedAlgorithm : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="UnsupportedAlgorithm"/> class.
        /// </summary>
        public UnsupportedAlgorithm(string message, Dictionary<string, object?>? details = null)
            : base("3300", message, details) { }
    }

    // ===== State Errors (4000-4999) =====

    /// <summary>
    /// Thrown when operation is invalid for current state.
    /// </summary>
    public sealed class InvalidStateException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="InvalidStateException"/> class.
        /// </summary>
        public InvalidStateException(string message, Dictionary<string, object?>? details = null)
            : base("4000", message, details) { }
    }

    /// <summary>
    /// Thrown when trying to use a closed document.
    /// </summary>
    public sealed class DocumentClosed : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="DocumentClosed"/> class.
        /// </summary>
        public DocumentClosed(string message, Dictionary<string, object?>? details = null)
            : base("4100", message, details) { }
    }

    /// <summary>
    /// Thrown when operation is not allowed in current state.
    /// </summary>
    public sealed class OperationNotAllowed : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="OperationNotAllowed"/> class.
        /// </summary>
        public OperationNotAllowed(string message, Dictionary<string, object?>? details = null)
            : base("4200", message, details) { }
    }

    /// <summary>
    /// Thrown when operation is invalid.
    /// </summary>
    public sealed class InvalidOperation : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="InvalidOperation"/> class.
        /// </summary>
        public InvalidOperation(string message, Dictionary<string, object?>? details = null)
            : base("4300", message, details) { }
    }

    // ===== Unsupported Feature Errors (5000-5999) =====

    /// <summary>
    /// Thrown when a feature is not supported.
    /// </summary>
    public sealed class UnsupportedFeatureException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="UnsupportedFeatureException"/> class.
        /// </summary>
        public UnsupportedFeatureException(string message, Dictionary<string, object?>? details = null)
            : base("5000", message, details) { }
    }

    /// <summary>
    /// Thrown when feature is not yet implemented.
    /// </summary>
    public sealed class FeatureNotImplemented : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="FeatureNotImplemented"/> class.
        /// </summary>
        public FeatureNotImplemented(string message, Dictionary<string, object?>? details = null)
            : base("5100", message, details) { }
    }

    /// <summary>
    /// Thrown when format is not supported.
    /// </summary>
    public sealed class FormatNotSupported : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="FormatNotSupported"/> class.
        /// </summary>
        public FormatNotSupported(string message, Dictionary<string, object?>? details = null)
            : base("5200", message, details) { }
    }

    /// <summary>
    /// Thrown when encoding is not supported.
    /// </summary>
    public sealed class EncodingNotSupported : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="EncodingNotSupported"/> class.
        /// </summary>
        public EncodingNotSupported(string message, Dictionary<string, object?>? details = null)
            : base("5300", message, details) { }
    }

    // ===== Validation Errors (6000-6999) =====

    /// <summary>
    /// Thrown when validation fails.
    /// </summary>
    public sealed class ValidationException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="ValidationException"/> class.
        /// </summary>
        public ValidationException(string message, Dictionary<string, object?>? details = null)
            : base("6000", message, details) { }
    }

    /// <summary>
    /// Thrown when parameter is invalid.
    /// </summary>
    public sealed class InvalidParameter : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="InvalidParameter"/> class.
        /// </summary>
        public InvalidParameter(string message, Dictionary<string, object?>? details = null)
            : base("6100", message, details) { }
    }

    /// <summary>
    /// Thrown when value is invalid.
    /// </summary>
    public sealed class InvalidValue : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="InvalidValue"/> class.
        /// </summary>
        public InvalidValue(string message, Dictionary<string, object?>? details = null)
            : base("6200", message, details) { }
    }

    /// <summary>
    /// Thrown when required field is missing.
    /// </summary>
    public sealed class MissingRequired : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="MissingRequired"/> class.
        /// </summary>
        public MissingRequired(string message, Dictionary<string, object?>? details = null)
            : base("6300", message, details) { }
    }

    /// <summary>
    /// Thrown when type doesn't match expected.
    /// </summary>
    public sealed class TypeMismatch : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="TypeMismatch"/> class.
        /// </summary>
        public TypeMismatch(string message, Dictionary<string, object?>? details = null)
            : base("6400", message, details) { }
    }

    // ===== Rendering Errors (7000-7999) =====

    /// <summary>
    /// Thrown when rendering fails.
    /// </summary>
    public sealed class RenderingException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="RenderingException"/> class.
        /// </summary>
        public RenderingException(string message, Dictionary<string, object?>? details = null)
            : base("7000", message, details) { }
    }

    /// <summary>
    /// Thrown when rendering operation fails.
    /// </summary>
    public sealed class RenderFailed : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="RenderFailed"/> class.
        /// </summary>
        public RenderFailed(string message, Dictionary<string, object?>? details = null)
            : base("7100", message, details) { }
    }

    /// <summary>
    /// Thrown when render format is not supported.
    /// </summary>
    public sealed class UnsupportedRenderFormat : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="UnsupportedRenderFormat"/> class.
        /// </summary>
        public UnsupportedRenderFormat(string message, Dictionary<string, object?>? details = null)
            : base("7200", message, details) { }
    }

    /// <summary>
    /// Thrown when there's not enough memory for rendering.
    /// </summary>
    public sealed class InsufficientMemory : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="InsufficientMemory"/> class.
        /// </summary>
        public InsufficientMemory(string message, Dictionary<string, object?>? details = null)
            : base("7300", message, details) { }
    }

    // ===== Search Errors (8000-8999) =====

    /// <summary>
    /// Thrown when search operations fail.
    /// </summary>
    public sealed class SearchException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="SearchException"/> class.
        /// </summary>
        public SearchException(string message, Dictionary<string, object?>? details = null)
            : base("8000", message, details) { }
    }

    /// <summary>
    /// Thrown when search operation fails.
    /// </summary>
    public sealed class SearchFailed : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="SearchFailed"/> class.
        /// </summary>
        public SearchFailed(string message, Dictionary<string, object?>? details = null)
            : base("8100", message, details) { }
    }

    /// <summary>
    /// Thrown when search pattern is invalid.
    /// </summary>
    public sealed class InvalidPattern : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="InvalidPattern"/> class.
        /// </summary>
        public InvalidPattern(string message, Dictionary<string, object?>? details = null)
            : base("8200", message, details) { }
    }

    /// <summary>
    /// Thrown when search index is corrupted.
    /// </summary>
    public sealed class IndexCorrupted : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="IndexCorrupted"/> class.
        /// </summary>
        public IndexCorrupted(string message, Dictionary<string, object?>? details = null)
            : base("8300", message, details) { }
    }

    // ===== Signature Errors (8500-8599) =====

    /// <summary>
    /// Thrown when digital signature operations fail.
    /// </summary>
    /// <remarks>
    /// Covers certificate loading errors, signing failures, verification failures,
    /// and credential management errors from the native FFI layer (error code 8).
    /// </remarks>
    public sealed class SignatureException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="SignatureException"/> class.
        /// </summary>
        public SignatureException(string message, Dictionary<string, object?>? details = null)
            : base("8500", message, details) { }

        /// <summary>
        /// Initializes a new instance of the <see cref="SignatureException"/> class with an inner exception.
        /// </summary>
        public SignatureException(string message, Exception? innerException, Dictionary<string, object?>? details = null)
            : base("8500", message, innerException, details) { }
    }

    // ===== Redaction Errors (8600-8699) =====

    /// <summary>
    /// Thrown when content redaction or metadata scrubbing fails.
    /// </summary>
    public sealed class RedactionException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="RedactionException"/> class.
        /// </summary>
        public RedactionException(string message, Dictionary<string, object?>? details = null)
            : base("8600", message, details) { }

        /// <summary>
        /// Initializes a new instance of the <see cref="RedactionException"/> class with an inner exception.
        /// </summary>
        public RedactionException(string message, Exception? innerException, Dictionary<string, object?>? details = null)
            : base("8600", message, innerException, details) { }
    }

    // ===== Accessibility Errors (9500-9599) =====

    /// <summary>
    /// Thrown when accessibility operations fail (tagging, structure tree, alt text).
    /// </summary>
    public sealed class AccessibilityException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="AccessibilityException"/> class.
        /// </summary>
        public AccessibilityException(string message, Dictionary<string, object?>? details = null)
            : base("9500", message, details) { }

        /// <summary>
        /// Initializes a new instance of the <see cref="AccessibilityException"/> class with an inner exception.
        /// </summary>
        public AccessibilityException(string message, Exception? innerException, Dictionary<string, object?>? details = null)
            : base("9500", message, innerException, details) { }
    }

    // ===== Optimization Errors (9600-9699) =====

    /// <summary>
    /// Thrown when optimization operations fail (font subsetting, image downsampling, deduplication).
    /// </summary>
    public sealed class OptimizationException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="OptimizationException"/> class.
        /// </summary>
        public OptimizationException(string message, Dictionary<string, object?>? details = null)
            : base("9600", message, details) { }

        /// <summary>
        /// Initializes a new instance of the <see cref="OptimizationException"/> class with an inner exception.
        /// </summary>
        public OptimizationException(string message, Exception? innerException, Dictionary<string, object?>? details = null)
            : base("9600", message, innerException, details) { }
    }

    // ===== Compliance Errors (9000-9100) =====

    /// <summary>
    /// Thrown when PDF compliance checks fail.
    /// </summary>
    public sealed class ComplianceException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="ComplianceException"/> class.
        /// </summary>
        public ComplianceException(string message, Dictionary<string, object?>? details = null)
            : base("9000", message, details) { }
    }

    /// <summary>
    /// Thrown when PDF is not compliant.
    /// </summary>
    public sealed class InvalidCompliance : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="InvalidCompliance"/> class.
        /// </summary>
        public InvalidCompliance(string message, Dictionary<string, object?>? details = null)
            : base("9100", message, details) { }
    }

    /// <summary>
    /// Thrown when PDF compliance validation or conversion fails.
    /// </summary>
    public sealed class ComplianceFailed : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="ComplianceFailed"/> class.
        /// </summary>
        public ComplianceFailed(string message, Dictionary<string, object?>? details = null)
            : base("9150", message, details) { }

        /// <summary>
        /// Initializes a new instance of the <see cref="ComplianceFailed"/> class with an inner exception.
        /// </summary>
        public ComplianceFailed(string message, Exception innerException, Dictionary<string, object?>? details = null)
            : base("9150", message, innerException, details) { }
    }

    /// <summary>
    /// Thrown when validation fails.
    /// </summary>
    public sealed class ValidationFailed : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="ValidationFailed"/> class.
        /// </summary>
        public ValidationFailed(string message, Dictionary<string, object?>? details = null)
            : base("9200", message, details) { }
    }

    // ===== OCR Errors (9200-9300) =====

    /// <summary>
    /// Thrown when OCR operations fail.
    /// </summary>
    public sealed class OcrException : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="OcrException"/> class.
        /// </summary>
        public OcrException(string message, Dictionary<string, object?>? details = null)
            : base("9200", message, details) { }
    }

    /// <summary>
    /// Thrown when text recognition fails.
    /// </summary>
    public sealed class RecognitionFailed : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="RecognitionFailed"/> class.
        /// </summary>
        public RecognitionFailed(string message, Dictionary<string, object?>? details = null)
            : base("9201", message, details) { }

        /// <summary>
        /// Initializes a new instance of the <see cref="RecognitionFailed"/> class with an inner exception.
        /// </summary>
        public RecognitionFailed(string message, Exception? innerException, Dictionary<string, object?>? details = null)
            : base("9201", message, innerException, details) { }
    }

    /// <summary>
    /// Thrown when language is not supported.
    /// </summary>
    public sealed class LanguageNotSupported : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="LanguageNotSupported"/> class.
        /// </summary>
        public LanguageNotSupported(string message, Dictionary<string, object?>? details = null)
            : base("9202", message, details) { }
    }

    /// <summary>
    /// Thrown when image processing fails.
    /// </summary>
    public sealed class ImageProcessingFailed : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="ImageProcessingFailed"/> class.
        /// </summary>
        public ImageProcessingFailed(string message, Dictionary<string, object?>? details = null)
            : base("9203", message, details) { }
    }

    // ===== Other Errors (9900-9999) =====

    /// <summary>
    /// Thrown when error type is unknown.
    /// </summary>
    public sealed class UnknownError : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="UnknownError"/> class.
        /// </summary>
        public UnknownError(string message, Dictionary<string, object?>? details = null)
            : base("9900", message, details) { }
    }

    /// <summary>
    /// Thrown for internal errors.
    /// </summary>
    public sealed class InternalError : PdfException
    {
        /// <summary>
        /// Initializes a new instance of the <see cref="InternalError"/> class.
        /// </summary>
        public InternalError(string message, Dictionary<string, object?>? details = null)
            : base("9901", message, details) { }
    }

    // ===== Error Mapping and Utilities =====

    /// <summary>
    /// Maps Rust error type names to C# exception types.
    /// </summary>
    public static class ErrorMapper
    {
        private static readonly Dictionary<string, Func<string, Dictionary<string, object?>?, PdfException>> ErrorMap =
            new()
            {
                // Parse errors
                { "InvalidStructure", (msg, details) => new InvalidStructure(msg, details) },
                { "CorruptedData", (msg, details) => new CorruptedData(msg, details) },
                { "UnsupportedVersion", (msg, details) => new UnsupportedVersion(msg, details) },

                // I/O errors
                { "FileNotFound", (msg, details) => new FileNotFound(msg, details) },
                { "PermissionDenied", (msg, details) => new PermissionDenied(msg, details) },
                { "DiskFull", (msg, details) => new DiskFull(msg, details) },
                { "NetworkError", (msg, details) => new NetworkError(msg, details) },

                // Encryption errors
                { "InvalidPassword", (msg, details) => new InvalidPassword(msg, details) },
                { "DecryptionFailed", (msg, details) => new DecryptionFailed(msg, details) },
                { "UnsupportedAlgorithm", (msg, details) => new UnsupportedAlgorithm(msg, details) },

                // State errors
                { "DocumentClosed", (msg, details) => new DocumentClosed(msg, details) },
                { "OperationNotAllowed", (msg, details) => new OperationNotAllowed(msg, details) },
                { "InvalidOperation", (msg, details) => new InvalidOperation(msg, details) },

                // Unsupported features
                { "FeatureNotImplemented", (msg, details) => new FeatureNotImplemented(msg, details) },
                { "FormatNotSupported", (msg, details) => new FormatNotSupported(msg, details) },
                { "EncodingNotSupported", (msg, details) => new EncodingNotSupported(msg, details) },

                // Validation errors
                { "InvalidParameter", (msg, details) => new InvalidParameter(msg, details) },
                { "InvalidValue", (msg, details) => new InvalidValue(msg, details) },
                { "MissingRequired", (msg, details) => new MissingRequired(msg, details) },
                { "TypeMismatch", (msg, details) => new TypeMismatch(msg, details) },

                // Rendering errors
                { "RenderFailed", (msg, details) => new RenderFailed(msg, details) },
                { "UnsupportedRenderFormat", (msg, details) => new UnsupportedRenderFormat(msg, details) },
                { "InsufficientMemory", (msg, details) => new InsufficientMemory(msg, details) },

                // Search errors
                { "SearchFailed", (msg, details) => new SearchFailed(msg, details) },
                { "InvalidPattern", (msg, details) => new InvalidPattern(msg, details) },
                { "IndexCorrupted", (msg, details) => new IndexCorrupted(msg, details) },

                // Signature errors
                { "SignatureException", (msg, details) => new SignatureException(msg, details) },

                // Redaction errors
                { "RedactionException", (msg, details) => new RedactionException(msg, details) },

                // Accessibility errors
                { "AccessibilityException", (msg, details) => new AccessibilityException(msg, details) },

                // Optimization errors
                { "OptimizationException", (msg, details) => new OptimizationException(msg, details) },

                // Other errors
                { "ComplianceException", (msg, details) => new ComplianceException(msg, details) },
                { "OcrException", (msg, details) => new OcrException(msg, details) },
                { "UnknownError", (msg, details) => new UnknownError(msg, details) },
            };

        /// <summary>
        /// Maps a Rust error type to a C# exception.
        /// </summary>
        /// <param name="rustErrorType">Rust error type name</param>
        /// <param name="message">Error message</param>
        /// <param name="details">Additional context</param>
        /// <returns>Appropriate PdfException subclass</returns>
        public static PdfException MapError(
            string rustErrorType,
            string message,
            Dictionary<string, object?>? details = null)
        {
            if (ErrorMap.TryGetValue(rustErrorType, out var factory))
            {
                return factory(message, details);
            }

            return new UnknownError(message, details);
        }
    }
}
