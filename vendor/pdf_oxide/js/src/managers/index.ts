/**
 * PDF Oxide Managers - Specialized facades for domain-specific operations
 *
 * This module provides manager classes that encapsulate domain-specific
 * operations on PDF documents, offering a cleaner and more organized API
 * compared to working directly with documents and pages.
 *
 * @example
 * ```typescript
 * import {
 *   OutlineManager,
 *   MetadataManager,
 *   ExtractionManager,
 *   SearchManager,
 *   SecurityManager,
 *   AnnotationManager,
 *   LayerManager,
 *   RenderingManager,
 * } from 'pdf_oxide';
 *
 * const doc = PdfDocument.open('document.pdf');
 *
 * // Metadata operations
 * const metadataManager = new MetadataManager(doc);
 * console.log(metadataManager.getTitle());
 *
 * // Text extraction
 * const extractionManager = new ExtractionManager(doc);
 * const text = extractionManager.extractAllText();
 *
 * // Search operations
 * const searchManager = new SearchManager(doc);
 * const results = searchManager.searchAll('keyword');
 *
 * // Page annotations
 * const page = doc.getPage(0);
 * const annotationManager = new AnnotationManager(page);
 * const highlights = annotationManager.getHighlights();
 * ```
 */

export {
  FieldVisibility,
  type FormField,
  type FormFieldConfig,
  FormFieldManager,
  FormFieldType,
} from '../form-field-manager.js';
// Phase 1 Expansion: Result Accessors and Forms
export {
  type AnnotationProperties,
  type FontProperties,
  type ImageProperties,
  ResultAccessorsManager,
  type SearchResultProperties,
} from '../result-accessors-manager.js';
export {
  AccessibilityManager,
  type AutoTagResult,
  type StructureElement,
  type StructureTree,
} from './accessibility-manager.js';
export {
  type Annotation,
  AnnotationManager,
  type AnnotationStatistics,
  type AnnotationValidation,
} from './annotation-manager.js';
export {
  BarcodeErrorCorrection,
  BarcodeFormat,
  type BarcodeGenerationConfig,
  BarcodeManager,
  type DetectedBarcode,
  QrErrorCorrection,
} from './barcode-manager.js';
// Phase 2.5: Batch Processing API
export {
  type BatchDocument,
  BatchManager,
  type BatchOptions,
  type BatchProgress,
  type BatchResult,
  type BatchStatistics,
} from './batch-manager.js';
export {
  CacheManager,
  type CacheStatistics as CacheStats,
} from './cache-manager.js';
export {
  type ComplianceIssue,
  ComplianceIssueType,
  ComplianceManager,
  type ComplianceValidationResult,
  IssueSeverity,
  PdfALevel,
  PdfUALevel,
  PdfXLevel,
} from './compliance-manager.js';
export {
  type ContentAnalysis,
  ContentManager,
} from './content-manager.js';
export {
  type ApplyRedactionsOptions,
  EditingManager,
  type RedactionRect,
  type RgbColor,
  type ScrubMetadataOptions,
} from './editing-manager.js';
export {
  BatesPosition,
  type Difference,
  DifferenceType,
  type DocumentComparisonResult,
  EnterpriseManager,
  type PageComparisonResult,
  StampAlignment,
} from './enterprise-manager.js';
export {
  type ContentStatistics,
  ExtractionManager,
  type SearchMatch,
} from './extraction-manager.js';
export {
  type Layer,
  type LayerHierarchy,
  LayerManager,
  type LayerStatistics,
  type LayerValidation,
} from './layer-manager.js';
export {
  type MetadataComparison,
  MetadataManager,
  type ValidationResult,
} from './metadata-manager.js';

// Canonical Managers (Phase 9 consolidation)
export {
  OCRManager,
  type OcrConfig,
  OcrDetectionMode,
  OcrManager,
  type OcrPageAnalysis,
  type OcrSpan,
} from './ocr-manager.js';
export {
  OptimizationManager,
  type OptimizationResult,
} from './optimization-manager.js';
// Core Managers
export { type OutlineItem, OutlineManager } from './outline-manager.js';
export {
  type PageInfo,
  PageManager,
  type PageRange,
  type PageStatistics,
} from './page-manager.js';
export {
  type PageBox,
  type PageDimensions,
  type PageResources,
  RenderingManager,
  type RenderingStatistics,
  RenderOptions,
  type RenderOptionsConfig,
} from './rendering-manager.js';
export {
  type SearchCapabilities,
  SearchManager,
  type SearchResult,
  type SearchStatistics,
} from './search-manager.js';
export {
  type AccessibilityValidation,
  type PermissionsSummary,
  type SecurityLevel,
  SecurityManager,
} from './security-manager.js';
export {
  type Certificate,
  type CertificateChain,
  CertificateFormat,
  type CertificateInfo,
  CertificationPermission,
  DigestAlgorithm,
  type DigitalSignature,
  FfiDigestAlgorithm,
  FfiSignatureSubFilter,
  type LoadedCertificate,
  type Signature,
  SignatureAlgorithm,
  type SignatureAppearance,
  type SignatureConfig,
  type SignatureField,
  type SignatureFieldConfig,
  SignatureManager,
  SignatureType,
  type SignatureValidationResult,
  type SigningCredentials,
  type SigningOptions,
  type SigningResult,
  type SignOptions,
  type TimestampConfig,
  type TimestampResult,
  TimestampStatus,
} from './signature-manager.js';
// Phase 2.4: Stream API support
export {
  createExtractionStream,
  createMetadataStream,
  createSearchStream,
  type ExtractionProgressData,
  ExtractionStream,
  MetadataStream,
  type PageMetadataData,
  type SearchResultData,
  SearchStream,
} from './streams.js';
export {
  XFAManager,
  XfaBindingType,
  type XfaCreationResult,
  type XfaDataOptions,
  type XfaDataset,
  type XfaField,
  type XfaFieldConfig,
  type XfaFieldHandle,
  XfaFieldType,
  XfaFormType,
  XfaManager,
  type XfaScriptConfig,
  type XfaSubformConfig,
  type XfaTemplateConfig,
  XfaValidationType,
} from './xfa-manager.js';
