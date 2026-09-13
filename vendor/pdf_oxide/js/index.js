// PDF Oxide Node.js bindings - Native module loader

import { platform, arch } from 'node:process';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import { dirname } from 'node:path';
import {
  PdfException,
  ParseException,
  IoException,
  EncryptionException,
  UnsupportedFeatureException,
  InvalidStateException,
  ValidationException,
  RenderingException,
  SearchException,
  ComplianceException,
  OcrException,
  UnknownError,
  wrapError,
  wrapMethod,
  wrapAsyncMethod,
} from './lib/errors.js';
import {
  addPdfDocumentProperties,
  addPdfProperties,
  addPdfPageProperties,
} from './lib/properties.js';
import {
  PdfBuilder,
  ConversionOptionsBuilder,
  MetadataBuilder,
  AnnotationBuilder,
  SearchOptionsBuilder,
} from './lib/builders/index.js';
import {
  OutlineManager,
  MetadataManager,
  ExtractionManager,
  SearchManager,
  SecurityManager,
  AnnotationManager,
  LayerManager,
  RenderingManager,
  OCRManager,
  OCRLanguage,
  OCRDetectionMode,
  ComplianceManager,
  PdfALevel,
  PdfXLevel,
  PdfUALevel,
  ComplianceIssueType,
  IssueSeverity,
  SignatureManager,
  SignatureAlgorithm,
  DigestAlgorithm,
  BarcodeManager,
  BarcodeFormat,
  BarcodeErrorCorrection,
  FormFieldManager,
  FormFieldType,
  FieldVisibility,
  ThumbnailManager,
  ThumbnailSize,
  ImageFormat,
  HybridMLManager,
  PageComplexity,
  ContentType,
  XfaManager,
  XfaFormType,
  XfaFieldType,
  SearchStream,
  ExtractionStream,
  MetadataStream,
  createSearchStream,
  createExtractionStream,
  createMetadataStream,
} from './lib/managers/index.js';

// Create require function for CommonJS modules
const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const require = createRequire(import.meta.url);

const PLATFORMS = {
  'darwin': {
    'x64': 'pdf_oxide-darwin-x64',
    'arm64': 'pdf_oxide-darwin-arm64',
  },
  'linux': {
    'x64': 'pdf_oxide-linux-x64-gnu',
    'arm64': 'pdf_oxide-linux-arm64-gnu',
  },
  'win32': {
    'x64': 'pdf_oxide-win32-x64-msvc',
    'arm64': 'pdf_oxide-win32-arm64-msvc',
  },
};

function getNativePackageName() {
  const osPackages = PLATFORMS[platform];
  if (!osPackages) {
    throw new Error(`Unsupported platform: ${platform}. Supported platforms: ${Object.keys(PLATFORMS).join(', ')}`);
  }

  const pkg = osPackages[arch];
  if (!pkg) {
    throw new Error(`Unsupported architecture: ${arch} for ${platform}. Supported architectures: ${Object.keys(osPackages).join(', ')}`);
  }

  return pkg;
}

let nativeModule;

function loadNativeModule() {
  if (nativeModule) {
    return nativeModule;
  }

  try {
    // Try loading from platform-specific package first (CommonJS)
    const packageName = getNativePackageName();
    try {
      // Use require to load the native module
      nativeModule = require(packageName);
    } catch (e) {
      // Fallback to local binary if in development
      if (process.env.NODE_ENV === 'development' || process.env.NAPI_DEV) {
        try {
          nativeModule = require('./pdf-oxide');
        } catch {
          throw e;
        }
      } else {
        throw e;
      }
    }
    return nativeModule;
  } catch (error) {
    throw new Error(`Failed to load native module: ${error.message}`);
  }
}

// Load native module
const native = loadNativeModule();

/**
 * Wraps native class methods to convert errors to proper JavaScript Error subclasses.
 * This ensures that errors thrown from native code are instanceof the appropriate Error class.
 *
 * @param {*} nativeClass - The native class to wrap
 * @param {string[]} [asyncMethods=[]] - Names of async methods to wrap specially
 * @returns {*} Wrapped class with error-handling methods
 */
function wrapNativeClass(nativeClass, asyncMethods = []) {
  if (!nativeClass) return nativeClass;

  // For static methods like PdfDocument.open()
  for (const key of Object.getOwnPropertyNames(nativeClass)) {
    if (key !== 'prototype' && key !== 'length' && key !== 'name' && typeof nativeClass[key] === 'function') {
      const isAsync = asyncMethods.includes(key);
      if (isAsync) {
        nativeClass[key] = wrapAsyncMethod(nativeClass[key], nativeClass);
      } else {
        nativeClass[key] = wrapMethod(nativeClass[key], nativeClass);
      }
    }
  }

  // For instance methods, wrap the prototype
  if (nativeClass.prototype) {
    for (const key of Object.getOwnPropertyNames(nativeClass.prototype)) {
      if (key !== 'constructor' && typeof nativeClass.prototype[key] === 'function') {
        const isAsync = asyncMethods.includes(key);
        const descriptor = Object.getOwnPropertyDescriptor(nativeClass.prototype, key);
        if (descriptor && descriptor.writable) {
          if (isAsync) {
            nativeClass.prototype[key] = wrapAsyncMethod(nativeClass.prototype[key]);
          } else {
            nativeClass.prototype[key] = wrapMethod(nativeClass.prototype[key]);
          }
        }
      }
    }
  }

  return nativeClass;
}

// List of async methods for each class (Phase 2.4 implementation)
const asyncMethodsByClass = {
  PdfDocument: [
    'extract_text_async',
    'to_markdown_async',
  ],
  Pdf: [
    'save_async',
  ],
  PdfBuilder: [],
  PdfPage: [],
  PdfElement: [],
  PdfText: [],
  PdfImage: [],
  PdfPath: [],
  PdfTable: [],
  PdfStructure: [],
  Annotation: [],
  TextAnnotation: [],
  HighlightAnnotation: [],
  LinkAnnotation: [],
  TextSearcher: [],
};

// Wrap native classes with error handling and property getters
const wrappedClasses = {};
for (const className of Object.keys(asyncMethodsByClass)) {
  if (native[className]) {
    wrappedClasses[className] = wrapNativeClass(native[className], asyncMethodsByClass[className]);
  }
}

// Add property getters to enhance idiomatic JavaScript API
if (wrappedClasses.PdfDocument) {
  addPdfDocumentProperties(wrappedClasses.PdfDocument);
}
if (wrappedClasses.Pdf) {
  addPdfProperties(wrappedClasses.Pdf);
}
if (wrappedClasses.PdfPage) {
  addPdfPageProperties(wrappedClasses.PdfPage);
}

// Export as ES module
const getVersion = native.getVersion;
const getPdfOxideVersion = native.getPdfOxideVersion;
const PdfDocument = wrappedClasses.PdfDocument || native.PdfDocument;
const Pdf = wrappedClasses.Pdf || native.Pdf;
// PdfBuilder is imported from ./lib/builders/index.js - don't redeclare
const PdfPage = wrappedClasses.PdfPage || native.PdfPage;
const PdfElement = wrappedClasses.PdfElement || native.PdfElement;
const PdfText = wrappedClasses.PdfText || native.PdfText;
const PdfImage = wrappedClasses.PdfImage || native.PdfImage;
const PdfPath = wrappedClasses.PdfPath || native.PdfPath;
const PdfTable = wrappedClasses.PdfTable || native.PdfTable;
const PdfStructure = wrappedClasses.PdfStructure || native.PdfStructure;
const Annotation = wrappedClasses.Annotation || native.Annotation;
const TextAnnotation = wrappedClasses.TextAnnotation || native.TextAnnotation;
const HighlightAnnotation = wrappedClasses.HighlightAnnotation || native.HighlightAnnotation;
const LinkAnnotation = wrappedClasses.LinkAnnotation || native.LinkAnnotation;
const PdfError = PdfException;
const PageSize = native.PageSize;
const Rect = native.Rect;
const Point = native.Point;
const Color = native.Color;
const ConversionOptions = native.ConversionOptions;
const SearchOptions = native.SearchOptions;
const SearchResult = native.SearchResult;
const TextSearcher = wrappedClasses.TextSearcher || native.TextSearcher;

export {
  // Version info
  getVersion,
  getPdfOxideVersion,

  // Main classes
  PdfDocument,
  Pdf,
  PdfPage,

  // Element types
  PdfElement,
  PdfText,
  PdfImage,
  PdfPath,
  PdfTable,
  PdfStructure,

  // Annotation types
  Annotation,
  TextAnnotation,
  HighlightAnnotation,
  LinkAnnotation,

  // Error types
  PdfError,
  PdfException,
  ParseException,
  IoException,
  EncryptionException,
  UnsupportedFeatureException,
  InvalidStateException,
  ValidationException,
  RenderingException,
  SearchException,
  ComplianceException,
  OcrException,
  UnknownError,

  // Types
  PageSize,
  Rect,
  Point,
  Color,
  ConversionOptions,
  SearchOptions,
  SearchResult,

  // Utilities
  TextSearcher,

  // Error utilities
  wrapError,
  wrapMethod,
  wrapAsyncMethod,

  // Builders
  PdfBuilder,
  ConversionOptionsBuilder,
  MetadataBuilder,
  AnnotationBuilder,
  SearchOptionsBuilder,

  // Managers (Phase 1-3: Core)
  OutlineManager,
  MetadataManager,
  ExtractionManager,
  SearchManager,
  SecurityManager,
  AnnotationManager,
  LayerManager,
  RenderingManager,

  // Managers (Phase 4+: TypeScript-compiled)
  OCRManager,
  OCRLanguage,
  OCRDetectionMode,
  ComplianceManager,
  PdfALevel,
  PdfXLevel,
  PdfUALevel,
  ComplianceIssueType,
  IssueSeverity,
  SignatureManager,
  SignatureAlgorithm,
  DigestAlgorithm,
  BarcodeManager,
  BarcodeFormat,
  BarcodeErrorCorrection,
  FormFieldManager,
  FormFieldType,
  FieldVisibility,
  ThumbnailManager,
  ThumbnailSize,
  ImageFormat,
  HybridMLManager,
  PageComplexity,
  ContentType,
  XfaManager,
  XfaFormType,
  XfaFieldType,

  // Phase 2.4: Stream API
  SearchStream,
  ExtractionStream,
  MetadataStream,
  createSearchStream,
  createExtractionStream,
  createMetadataStream,
};
