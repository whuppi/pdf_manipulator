/**
 * Unified Error Handling for PDF Oxide
 *
 * Provides comprehensive exception hierarchy consistent across all language bindings.
 * Uses 4-digit error codes organized by category (1000-9999).
 *
 * Error Code System:
 * - 1000-1999: Parse errors
 * - 2000-2999: I/O errors
 * - 3000-3999: Encryption errors
 * - 4000-4999: State errors
 * - 5000-5999: Unsupported feature errors
 * - 6000-6999: Validation errors
 * - 7000-7999: Rendering errors
 * - 8000-8999: Search errors
 * - 9000-9999: Other errors (compliance, OCR, etc.)
 */

import type { PdfErrorDetails } from './types/common';

/**
 * Error categories for classification and handling
 */
export enum ErrorCategory {
  VALIDATION = 'validation',
  IO = 'io',
  ENCRYPTION = 'encryption',
  PARSING = 'parsing',
  RENDERING = 'rendering',
  SEARCH = 'search',
  PERMISSION = 'permission',
  RESOURCE = 'resource',
  STATE = 'state',
  UNSUPPORTED = 'unsupported',
  COMPLIANCE = 'compliance',
  OCR = 'ocr',
  SIGNATURE = 'signature',
  REDACTION = 'redaction',
  UNKNOWN = 'unknown',
}

/**
 * Error severity levels for prioritization and monitoring
 */
export enum ErrorSeverity {
  LOW = 'low',
  MEDIUM = 'medium',
  HIGH = 'high',
  CRITICAL = 'critical',
}

/**
 * Recovery information for error handling
 */
export interface ErrorRecovery {
  canRetry: boolean;
  retryAfterMs?: number;
  suggestions: string[];
  alternativeApproach?: string;
}

/**
 * Base class for all PDF Oxide errors.
 *
 * @class PdfException
 * @extends {Error}
 * @property {string} code - 4-digit error code (XXXX format)
 * @property {string} message - Human-readable error message
 * @property {PdfErrorDetails} details - Additional context information
 *
 * @example
 * try {
 *   // PDF operation
 * } catch (err) {
 *   if (err instanceof PdfException) {
 *     console.log(`[${err.code}] ${err.message}`);
 *     console.log('Context:', err.details);
 *   }
 * }
 */
export class PdfException extends Error {
  public readonly code: string;
  public readonly message: string;
  public readonly details: PdfErrorDetails;
  public readonly category: ErrorCategory;
  public readonly severity: ErrorSeverity;
  public readonly recovery: ErrorRecovery;

  /**
   * Creates a new PdfException.
   *
   * @param code - 4-digit error code
   * @param message - Human-readable error message
   * @param category - Error category for classification
   * @param severity - Error severity level
   * @param details - Additional context information
   * @param recovery - Recovery information
   */
  constructor(
    code: string,
    message: string,
    category: ErrorCategory = ErrorCategory.UNKNOWN,
    severity: ErrorSeverity = ErrorSeverity.HIGH,
    details: PdfErrorDetails = {},
    recovery: Partial<ErrorRecovery> = {}
  ) {
    if (!code || code.length !== 4 || !/^\d{4}$/.test(code)) {
      throw new Error('Code must be 4 digits');
    }

    super(`[${code}] ${message}`);
    this.name = this.constructor.name;
    this.code = code;
    this.message = message;
    this.category = category;
    this.severity = severity;
    this.details = {
      timestamp: new Date().toISOString(),
      ...details,
    };
    this.recovery = {
      canRetry: false,
      suggestions: [],
      ...recovery,
    };

    // Maintain proper stack trace for where our error was thrown
    if (Error.captureStackTrace) {
      Error.captureStackTrace(this, this.constructor);
    }
  }

  /**
   * Adds operational context to this exception for better diagnostics.
   *
   * @param operation - Name of the operation that failed
   * @param context - Additional context key-value pairs
   * @returns This exception for method chaining
   *
   * @example
   * throw new FileNotFound('Not found')
   *   .withContext('openFile', { path: '/tmp/doc.pdf' });
   */
  withContext(operation: string, context: Record<string, any> = {}): this {
    this.details.operation = operation;
    this.details.context = { ...this.details.context, ...context };
    return this;
  }

  /**
   * Get a comprehensive error message with all details
   * @returns Formatted error message with context and suggestions
   */
  getFullMessage(): string {
    const parts: string[] = [
      `[${this.code}] ${this.message}`,
      `Category: ${this.category} | Severity: ${this.severity}`,
    ];

    if (this.details.operation) {
      parts.push(`Operation: ${this.details.operation}`);
    }

    if (this.details.context && Object.keys(this.details.context).length > 0) {
      const contextStr = Object.entries(this.details.context)
        .map(([key, value]) => `  ${key}: ${JSON.stringify(value)}`)
        .join('\n');
      parts.push(`Context:\n${contextStr}`);
    }

    if (this.recovery.suggestions.length > 0) {
      const suggestionsStr = this.recovery.suggestions.map((s) => `  • ${s}`).join('\n');
      parts.push(`Recovery Suggestions:\n${suggestionsStr}`);
    }

    if (this.recovery.alternativeApproach) {
      parts.push(`Alternative Approach: ${this.recovery.alternativeApproach}`);
    }

    if (this.recovery.canRetry) {
      const retryMsg = this.recovery.retryAfterMs
        ? `retry after ${this.recovery.retryAfterMs}ms`
        : 'retry';
      parts.push(`Status: Retryable (${retryMsg})`);
    }

    return parts.join('\n');
  }

  /**
   * Convert error to JSON for logging/monitoring
   * @returns JSON representation of the error
   */
  toJSON() {
    return {
      name: this.name,
      code: this.code,
      message: this.message,
      category: this.category,
      severity: this.severity,
      details: this.details,
      recovery: this.recovery,
      stack: this.stack,
    };
  }
}

// ===== Parse Errors (1000-1999) =====

export class ParseException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('1000', message, ErrorCategory.PARSING, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'Verify the PDF file is not corrupted',
        'Try opening the file with Adobe Reader',
        'Check that the file is a valid PDF',
      ],
    });
  }
}

export class InvalidStructure extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('1101', message, ErrorCategory.PARSING, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'The PDF structure is invalid or incomplete',
        'Try re-saving the PDF with a PDF tool',
        'Contact the PDF creator for a corrected version',
      ],
    });
  }
}

export class CorruptedData extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('1200', message, ErrorCategory.PARSING, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'PDF file may be corrupted or damaged',
        'Try opening the file with Adobe Reader',
        'Attempt to recover the file using a PDF recovery tool',
      ],
    });
  }
}

export class UnsupportedVersion extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('1300', message, ErrorCategory.UNSUPPORTED, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'This PDF version is not supported',
        'Try converting the PDF to a standard format',
        'Update your PDF library to a newer version',
      ],
      alternativeApproach: 'Use a PDF converter tool to convert to standard PDF format',
    });
  }
}

// ===== I/O Errors (2000-2999) =====

export class IoException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('2000', message, ErrorCategory.IO, ErrorSeverity.HIGH, details, {
      canRetry: true,
      retryAfterMs: 1000,
      suggestions: [
        'Check file permissions (readable)',
        'Verify file path exists',
        'Ensure disk space available',
        'Check file is not locked by another process',
      ],
    });
  }
}

export class FileNotFound extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('2100', message, ErrorCategory.IO, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'Verify the file path is correct',
        'Check that the file exists',
        'Ensure the path is absolute or properly resolved',
      ],
    });
  }
}

export class PermissionDenied extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('2200', message, ErrorCategory.PERMISSION, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'Check file permissions using chmod/ls -l',
        'Ensure the process has read permissions',
        'Try running with appropriate permissions',
      ],
      alternativeApproach: 'Copy the file to a location with proper permissions',
    });
  }
}

export class DiskFull extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('2300', message, ErrorCategory.RESOURCE, ErrorSeverity.CRITICAL, details, {
      canRetry: true,
      retryAfterMs: 2000,
      suggestions: [
        'Free up disk space',
        'Delete unnecessary files',
        'Archive old files to external storage',
      ],
      alternativeApproach: 'Process the PDF on a device with more available storage',
    });
  }
}

export class NetworkError extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('2400', message, ErrorCategory.IO, ErrorSeverity.MEDIUM, details, {
      canRetry: true,
      retryAfterMs: 2000,
      suggestions: [
        'Check network connectivity',
        'Verify the remote server is reachable',
        'Check firewall and proxy settings',
      ],
    });
  }
}

// ===== Encryption Errors (3000-3999) =====

export class EncryptionException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('3000', message, ErrorCategory.ENCRYPTION, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'Check if PDF requires password',
        'Verify encryption credentials',
        'Try opening PDF in Adobe Reader to verify',
      ],
    });
  }
}

export class InvalidPassword extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('3100', message, ErrorCategory.ENCRYPTION, ErrorSeverity.MEDIUM, details, {
      canRetry: true,
      retryAfterMs: 1000,
      suggestions: [
        'Verify the password is correct',
        'Check for leading/trailing spaces',
        'Ensure correct keyboard layout is selected',
        'Try password with different case',
      ],
    });
  }
}

export class DecryptionFailed extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('3200', message, ErrorCategory.ENCRYPTION, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'File may be corrupted',
        'Encryption method may be unsupported',
        'Try opening with different PDF reader',
      ],
    });
  }
}

export class UnsupportedAlgorithm extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('3300', message, ErrorCategory.ENCRYPTION, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'This encryption algorithm is not supported',
        'Convert PDF with different encryption algorithm',
        'Contact PDF creator for unencrypted version',
      ],
      alternativeApproach: 'Use Adobe Acrobat to re-save with supported encryption',
    });
  }
}

// ===== State Errors (4000-4999) =====

export class InvalidStateException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('4000', message, ErrorCategory.STATE, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'Ensure the document is in the correct state for this operation',
        'Check that required initialization steps were completed',
        'Verify operation order and dependencies',
      ],
    });
  }
}

export class DocumentClosed extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('4100', message, ErrorCategory.STATE, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'The document has been closed and cannot be used',
        'Create a new document instance or reopen the file',
        'Check that the document is not being used after closure',
      ],
    });
  }
}

export class OperationNotAllowed extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('4200', message, ErrorCategory.STATE, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'This operation is not allowed in the current document state',
        'Check document permissions and security settings',
        'Ensure document is writable (not read-only)',
      ],
    });
  }
}

export class InvalidOperation extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('4300', message, ErrorCategory.STATE, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'This operation cannot be performed on this document',
        'Verify the document format and content type',
        'Check operation prerequisites are met',
      ],
    });
  }
}

// ===== Unsupported Feature Errors (5000-5999) =====

export class UnsupportedFeatureException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('5000', message, ErrorCategory.UNSUPPORTED, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'This PDF feature is not supported',
        'Check the documentation for supported features',
        'Consider using a different PDF tool for this operation',
      ],
    });
  }
}

export class FeatureNotImplemented extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('5100', message, ErrorCategory.UNSUPPORTED, ErrorSeverity.LOW, details, {
      canRetry: false,
      suggestions: [
        'This feature is not yet implemented',
        'Check the library changelog for planned features',
        'File a feature request if this is important for your use case',
      ],
    });
  }
}

export class FormatNotSupported extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('5200', message, ErrorCategory.UNSUPPORTED, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'The document format is not supported',
        'Try converting to a standard format first',
        'Check documentation for supported formats',
      ],
      alternativeApproach: 'Convert the document using a format converter',
    });
  }
}

export class EncodingNotSupported extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('5300', message, ErrorCategory.UNSUPPORTED, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'The character encoding is not supported',
        'Try using a different character encoding',
        'Check that the PDF text is properly encoded',
      ],
    });
  }
}

// ===== Validation Errors (6000-6999) =====

export class ValidationException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('6000', message, ErrorCategory.VALIDATION, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'Verify all parameters are valid and properly formatted',
        'Check the API documentation for parameter requirements',
        'Ensure all required fields are provided',
      ],
    });
  }
}

export class InvalidParameter extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('6100', message, ErrorCategory.VALIDATION, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'Check that all parameters are valid',
        'Review the function signature and parameter types',
        'Ensure parameter values are within valid ranges',
      ],
    });
  }
}

export class InvalidValue extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('6200', message, ErrorCategory.VALIDATION, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'Verify the value is correct and properly formatted',
        'Check acceptable value ranges in the documentation',
        'Ensure the value type matches the expected type',
      ],
    });
  }
}

export class MissingRequired extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('6300', message, ErrorCategory.VALIDATION, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'Provide the required parameter or field',
        'Check the API documentation for required fields',
        'Ensure no required parameters are omitted',
      ],
    });
  }
}

export class TypeMismatch extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('6400', message, ErrorCategory.VALIDATION, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'Verify the parameter type matches the expected type',
        'Convert the value to the correct type',
        'Check the function signature in the documentation',
      ],
    });
  }
}

// ===== Rendering Errors (7000-7999) =====

export class RenderingException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('7000', message, ErrorCategory.RENDERING, ErrorSeverity.HIGH, details, {
      canRetry: true,
      retryAfterMs: 1000,
      suggestions: [
        'Check that the PDF content is valid',
        'Verify rendering parameters are correct',
        'Try rendering a different page to isolate the issue',
      ],
    });
  }
}

export class RenderFailed extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('7100', message, ErrorCategory.RENDERING, ErrorSeverity.HIGH, details, {
      canRetry: true,
      retryAfterMs: 1000,
      suggestions: [
        'The rendering operation failed',
        'Check available system resources',
        'Try rendering with lower quality settings',
      ],
    });
  }
}

export class UnsupportedRenderFormat extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('7200', message, ErrorCategory.RENDERING, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'The render format is not supported',
        'Use a supported format (PNG, JPG, PDF, SVG)',
        'Check documentation for supported rendering formats',
      ],
    });
  }
}

export class InsufficientMemory extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('7300', message, ErrorCategory.RESOURCE, ErrorSeverity.CRITICAL, details, {
      canRetry: true,
      retryAfterMs: 2000,
      suggestions: [
        'Free up system memory',
        'Close other applications to reduce memory usage',
        'Reduce render resolution or quality',
      ],
      alternativeApproach: 'Process the document on a system with more available memory',
    });
  }
}

// ===== Search Errors (8000-8999) =====

export class SearchException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('8000', message, ErrorCategory.SEARCH, ErrorSeverity.MEDIUM, details, {
      canRetry: true,
      retryAfterMs: 500,
      suggestions: [
        'Verify the search term is valid',
        'Check that the document contains searchable text',
        'Try a simpler search pattern',
      ],
    });
  }
}

export class SearchFailed extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('8100', message, ErrorCategory.SEARCH, ErrorSeverity.MEDIUM, details, {
      canRetry: true,
      retryAfterMs: 500,
      suggestions: [
        'The search operation failed',
        'Verify the document is not corrupted',
        'Check that the search parameters are valid',
      ],
    });
  }
}

export class InvalidPattern extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('8200', message, ErrorCategory.VALIDATION, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'The search pattern is invalid',
        'Check the regular expression syntax',
        'Escape special characters if needed',
      ],
    });
  }
}

export class IndexCorrupted extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('8300', message, ErrorCategory.SEARCH, ErrorSeverity.HIGH, details, {
      canRetry: true,
      retryAfterMs: 1000,
      suggestions: [
        'The search index may be corrupted',
        'Try rebuilding the search index',
        'Verify the PDF document is valid',
      ],
    });
  }
}

// ===== Signature Errors (8500-8599) =====

export class SignatureException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('8500', message, ErrorCategory.SIGNATURE, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'Verify the certificate file path and password are correct',
        'Ensure the certificate contains a valid private key for signing',
        'Check that the PDF data is valid and not corrupted',
        'Confirm the signing algorithm is supported',
      ],
    });
  }
}

export class CertificateLoadFailed extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('8501', message, ErrorCategory.SIGNATURE, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'Verify the certificate file exists and is readable',
        'Check the password for PKCS#12 files',
        'Ensure the PEM files are properly formatted',
        'Confirm the certificate and key match',
      ],
    });
  }
}

export class SigningFailed extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('8502', message, ErrorCategory.SIGNATURE, ErrorSeverity.HIGH, details, {
      canRetry: true,
      retryAfterMs: 1000,
      suggestions: [
        'Verify the credentials have a valid private key',
        'Check that the PDF data is not corrupted',
        'Ensure the signing algorithm is compatible with the certificate',
        'Try a different signature subfilter',
      ],
    });
  }
}

// ===== Redaction Errors (8600-8699) =====

export class RedactionException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('8600', message, ErrorCategory.REDACTION, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'Verify the redaction area coordinates are valid',
        'Ensure the document is opened for editing',
        'Check that the page index is within range',
        'Confirm the document is not read-only or encrypted',
      ],
    });
  }
}

// ===== Accessibility Errors (9500-9599) =====

export class AccessibilityException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('9500', message, ErrorCategory.VALIDATION, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'Check the document structure and tagging',
        'Verify alt text is set for images and figures',
        'Ensure document language and title are specified',
        'Run accessibility validation to identify issues',
      ],
    });
  }
}

// ===== Optimization Errors (9600-9699) =====

export class OptimizationException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('9600', message, ErrorCategory.RESOURCE, ErrorSeverity.MEDIUM, details, {
      canRetry: true,
      retryAfterMs: 1000,
      suggestions: [
        'Check that the document is not corrupted',
        'Verify sufficient disk space for optimization',
        'Try optimizing with different settings (DPI, quality)',
        'Ensure document is not encrypted or read-only',
      ],
    });
  }
}

// ===== Compliance Errors (9000-9100) =====

export class ComplianceException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('9000', message, ErrorCategory.COMPLIANCE, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'Check PDF compliance level requirements',
        'Verify document meets compliance standards',
        'Review compliance validation report',
      ],
    });
  }
}

export class InvalidCompliance extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('9100', message, ErrorCategory.COMPLIANCE, ErrorSeverity.HIGH, details, {
      canRetry: false,
      suggestions: [
        'The document does not meet compliance requirements',
        'Fix the compliance issues identified',
        'Convert the document to a compliant format',
      ],
    });
  }
}

export class ValidationFailed extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('9150', message, ErrorCategory.COMPLIANCE, ErrorSeverity.MEDIUM, details, {
      canRetry: true,
      retryAfterMs: 1000,
      suggestions: [
        'Validation failed for this document',
        'Check the validation error details',
        'Fix the identified issues and retry',
      ],
    });
  }
}

// ===== OCR Errors (9200-9300) =====

export class OcrException extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('9200', message, ErrorCategory.OCR, ErrorSeverity.MEDIUM, details, {
      canRetry: true,
      retryAfterMs: 2000,
      suggestions: [
        'OCR processing failed',
        'Verify the image quality is sufficient',
        'Check that the document language is supported',
      ],
    });
  }
}

export class RecognitionFailed extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('9201', message, ErrorCategory.OCR, ErrorSeverity.MEDIUM, details, {
      canRetry: true,
      retryAfterMs: 2000,
      suggestions: [
        'Text recognition failed',
        'Try improving the image quality',
        'Ensure the document language is set correctly',
      ],
    });
  }
}

export class LanguageNotSupported extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('9202', message, ErrorCategory.OCR, ErrorSeverity.MEDIUM, details, {
      canRetry: false,
      suggestions: [
        'The document language is not supported for OCR',
        'Check available language packs',
        'Install language support if available',
      ],
    });
  }
}

export class ImageProcessingFailed extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('9203', message, ErrorCategory.OCR, ErrorSeverity.MEDIUM, details, {
      canRetry: true,
      retryAfterMs: 2000,
      suggestions: [
        'Image processing failed',
        'Verify image format and encoding',
        'Try with a higher quality image',
      ],
    });
  }
}

// ===== Other Errors (9900-9999) =====

export class UnknownError extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('9900', message, ErrorCategory.UNKNOWN, ErrorSeverity.HIGH, details, {
      canRetry: true,
      retryAfterMs: 1000,
      suggestions: [
        'An unknown error occurred',
        'Check the error message for details',
        'Try the operation again',
        'File a bug report if the problem persists',
      ],
    });
  }
}

export class InternalError extends PdfException {
  constructor(message: string, details: PdfErrorDetails = {}) {
    super('9901', message, ErrorCategory.UNKNOWN, ErrorSeverity.CRITICAL, details, {
      canRetry: true,
      retryAfterMs: 2000,
      suggestions: [
        'An internal error occurred',
        'Please restart the application',
        'Check system resources',
        'File a bug report with error details',
      ],
    });
  }
}

// ===== Error Mapping from Rust FFI =====

/**
 * Maps Rust error type names to JavaScript exception types.
 */
export const ERROR_MAP: Record<
  string,
  new (
    message: string,
    details?: PdfErrorDetails
  ) => PdfException
> = {
  // Parse errors
  InvalidStructure,
  CorruptedData,
  UnsupportedVersion,

  // I/O errors
  FileNotFound,
  PermissionDenied,
  DiskFull,
  NetworkError,

  // Encryption errors
  InvalidPassword,
  DecryptionFailed,
  UnsupportedAlgorithm,

  // State errors
  DocumentClosed,
  OperationNotAllowed,
  InvalidOperation,

  // Unsupported features
  FeatureNotImplemented,
  FormatNotSupported,
  EncodingNotSupported,

  // Validation errors
  InvalidParameter,
  InvalidValue,
  MissingRequired,
  TypeMismatch,

  // Rendering errors
  RenderFailed,
  UnsupportedRenderFormat,
  InsufficientMemory,

  // Search errors
  SearchFailed,
  InvalidPattern,
  IndexCorrupted,

  // Signature errors
  SignatureException,
  CertificateLoadFailed,
  SigningFailed,

  // Redaction errors
  RedactionException,

  // Accessibility errors
  AccessibilityException,

  // Optimization errors
  OptimizationException,

  // Other errors
  ComplianceException,
  OcrException,
  UnknownError,
};

/**
 * Maps a Rust error type to a JavaScript exception.
 *
 * @param rustErrorType - Rust error type name
 * @param message - Error message
 * @param details - Additional context
 * @returns Appropriate error instance
 *
 * @example
 * try {
 *   // Native call
 * } catch (err) {
 *   const jsErr = mapError('FileNotFound', 'File does not exist', { path: '/tmp/doc.pdf' });
 *   throw jsErr;
 * }
 */
export function mapError(
  rustErrorType: string,
  message: string,
  details: PdfErrorDetails = {}
): PdfException {
  const ErrorClass = ERROR_MAP[rustErrorType] || UnknownError;
  return new ErrorClass(message, details);
}

/**
 * Maps a numeric FFI error code from the Rust layer to a JavaScript exception.
 *
 * FFI Error Codes:
 * - 0: Success (no error)
 * - 1: I/O error
 * - 2: Parse error
 * - 3: Encryption error
 * - 4: Invalid state error
 * - 5: Rendering unsupported
 * - 6: OCR unsupported
 * - 7: Invalid argument
 * - 8: Signature error
 * - 100: Internal/generic error
 *
 * @param errorCode - Numeric FFI error code from native layer
 * @param message - Optional error message override
 * @returns Appropriate error instance
 *
 * @example
 * const errorCode = nativeCall();
 * if (errorCode !== 0) {
 *   throw mapFfiErrorCode(errorCode, 'Operation failed');
 * }
 */
export function mapFfiErrorCode(errorCode: number, message?: string): PdfException {
  switch (errorCode) {
    case 0:
      return new UnknownError(message ?? 'Success (no error)');
    case 1:
      return new IoException(
        message ?? 'I/O error: File not found, permission denied, or read/write failed'
      );
    case 2:
      return new ParseException(message ?? 'Parse error: Invalid PDF structure or content stream');
    case 3:
      return new EncryptionException(
        message ?? 'Encryption error: Incorrect password or unsupported encryption'
      );
    case 4:
      return new InvalidStateException(
        message ?? 'Invalid state: Operation not allowed in current document state'
      );
    case 5:
      return new UnsupportedFeatureException(
        message ?? 'Feature not enabled: Rendering support not compiled in'
      );
    case 6:
      return new UnsupportedFeatureException(
        message ?? 'Feature not enabled: OCR support not compiled in'
      );
    case 7:
      return new InvalidParameter(
        message ?? 'Invalid argument: Null pointer or invalid parameter passed'
      );
    case 8:
      return new SignatureException(
        message ?? 'Signature error: Certificate loading, signing, or verification failed'
      );
    case 9:
      return new RedactionException(
        message ?? 'Redaction error: Content redaction or metadata scrubbing failed'
      );
    case 10:
      return new ComplianceException(
        message ?? 'Compliance error: PDF/A, PDF/X, or PDF/UA conversion or validation failed'
      );
    case 11:
      return new AccessibilityException(
        message ?? 'Accessibility error: Tagging, structure tree, or alt text operation failed'
      );
    case 12:
      return new OptimizationException(
        message ??
          'Optimization error: Font subsetting, image downsampling, or deduplication failed'
      );
    default:
      return new UnknownError(message ?? `Unknown error (code: ${errorCode})`);
  }
}

/**
 * Creates an error with optional context.
 *
 * @param code - 4-digit error code
 * @param message - Error message
 * @param options - Configuration options
 * @returns Created exception
 *
 * @example
 * const err = createError('7100', 'Rendering failed', {
 *   operation: 'renderPage',
 *   context: { page: 0, format: 'png' }
 * });
 */
export function createError(
  code: string,
  message: string,
  options: {
    operation?: string;
    context?: Record<string, any>;
  } = {}
): PdfException {
  // Map code to appropriate error class
  const codeMap: Record<string, new (message: string, details?: PdfErrorDetails) => PdfException> =
    {
      '1101': InvalidStructure,
      '1200': CorruptedData,
      '1300': UnsupportedVersion,
      '2100': FileNotFound,
      '2200': PermissionDenied,
      '2300': DiskFull,
      '2400': NetworkError,
      '3100': InvalidPassword,
      '3200': DecryptionFailed,
      '3300': UnsupportedAlgorithm,
      '4100': DocumentClosed,
      '4200': OperationNotAllowed,
      '4300': InvalidOperation,
      '5100': FeatureNotImplemented,
      '5200': FormatNotSupported,
      '5300': EncodingNotSupported,
      '6100': InvalidParameter,
      '6200': InvalidValue,
      '6300': MissingRequired,
      '6400': TypeMismatch,
      '7100': RenderFailed,
      '7200': UnsupportedRenderFormat,
      '7300': InsufficientMemory,
      '8100': SearchFailed,
      '8200': InvalidPattern,
      '8300': IndexCorrupted,
      '8500': SignatureException,
      '8501': CertificateLoadFailed,
      '8502': SigningFailed,
      '8600': RedactionException,
      '9500': AccessibilityException,
      '9600': OptimizationException,
      '9100': InvalidCompliance,
      '9150': ValidationFailed,
      '9200': OcrException,
      '9201': RecognitionFailed,
      '9202': LanguageNotSupported,
      '9203': ImageProcessingFailed,
    };

  const ErrorClass = codeMap[code];
  const context = options.context || {};

  let err: PdfException;
  if (ErrorClass) {
    err = new ErrorClass(message, context);
  } else {
    err = new PdfException(code, message, ErrorCategory.UNKNOWN, ErrorSeverity.HIGH, context);
  }

  if (options.operation) {
    err.withContext(options.operation, context);
  }

  return err;
}

/**
 * Maps native error to appropriate PDF exception.
 *
 * @param error - The error to wrap
 * @returns Wrapped exception
 *
 * @example
 * try {
 *   // Native call
 * } catch (err) {
 *   const wrapped = wrapError(err);
 *   console.log(wrapped.code); // e.g., "2100"
 * }
 */
export function wrapError(error: unknown): PdfException {
  // If already a PdfException, return as-is
  if (error instanceof PdfException) {
    return error;
  }

  let code = '9900';
  let message = 'Unknown error occurred';
  let details: PdfErrorDetails = {};

  // Handle Error objects
  if (error instanceof Error) {
    message = error.message;

    // Try to extract code from message format: [XXXX] message
    const codeMatch = message.match(/^\[(\d{4})\]/);
    if (codeMatch && codeMatch[1]) {
      code = codeMatch[1];
      message = message.replace(/^\[\d{4}\]\s*/, '');
    }

    // Copy additional properties
    const props = Object.getOwnPropertyNames(error);
    for (const prop of props) {
      if (prop !== 'message' && prop !== 'name' && prop !== 'stack') {
        details[prop as keyof PdfErrorDetails] = (error as any)[prop];
      }
    }
  }
  // Handle plain objects
  else if (error && typeof error === 'object') {
    const obj = error as Record<string, any>;
    if (obj.code) {
      code = String(obj.code);
      // Convert string codes to 4-digit if needed
      if (!/^\d{4}$/.test(code)) {
        code = '9900';
      }
    }
    if (obj.message && typeof obj.message === 'string') {
      message = obj.message;
    }
    details = obj as PdfErrorDetails;
  }
  // Handle strings
  else if (typeof error === 'string') {
    message = error;
  }

  try {
    return new PdfException(code, message, ErrorCategory.UNKNOWN, ErrorSeverity.HIGH, details);
  } catch {
    // If code validation fails, use generic error
    return new UnknownError(message, details);
  }
}

/**
 * Function signature for methods to be wrapped
 */
type MethodFunction = (...args: any[]) => any;
type AsyncMethodFunction = (...args: any[]) => Promise<any>;

/**
 * Creates a method wrapper that catches native errors and converts them.
 *
 * @param fn - The method to wrap
 * @param thisArg - The context (this) to bind
 * @returns Wrapped function with error conversion
 *
 * @example
 * const wrapped = wrapMethod(nativeMethod, this);
 * const result = wrapped(arg1, arg2); // Throws PdfException on error
 */
export function wrapMethod<T extends MethodFunction>(fn: T, thisArg: any = null): T {
  return function (this: any, ...args: any[]) {
    try {
      return fn.apply(thisArg || this, args);
    } catch (nativeErr) {
      throw wrapError(nativeErr);
    }
  } as T;
}

/**
 * Creates an async method wrapper that catches native errors.
 *
 * @param fn - The async method to wrap
 * @param thisArg - The context (this) to bind
 * @returns Wrapped async function with error conversion
 *
 * @example
 * const wrapped = wrapAsyncMethod(nativeAsyncMethod, this);
 * const result = await wrapped(arg1, arg2); // Throws PdfException on error
 */
export function wrapAsyncMethod<T extends AsyncMethodFunction>(fn: T, thisArg: any = null): T {
  return async function (this: any, ...args: any[]) {
    try {
      return await fn.apply(thisArg || this, args);
    } catch (nativeErr) {
      throw wrapError(nativeErr);
    }
  } as T;
}
