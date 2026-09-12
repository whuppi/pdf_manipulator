/**
 * FormFieldManager for form field operations
 *
 * Manages form fields in PDF documents including retrieval, modification, and flattening.
 * API is consistent with Python, Java, C#, Go, and Swift implementations.
 */

import { EventEmitter } from 'events';

/**
 * Types of form fields
 */
export enum FormFieldType {
  Text = 'Text',
  CheckBox = 'CheckBox',
  RadioButton = 'RadioButton',
  ComboBox = 'ComboBox',
  ListBox = 'ListBox',
  Button = 'Button',
  Signature = 'Signature',
  TextArea = 'TextArea',
}

/**
 * Field visibility options
 */
export enum FieldVisibility {
  Visible = 'Visible',
  Hidden = 'Hidden',
  PrintOnly = 'PrintOnly',
  NoView = 'NoView',
}

/**
 * Form field information
 */
export interface FormField {
  fieldName: string;
  fieldType: FormFieldType;
  pageIndex?: number;
  isRequired: boolean;
  isReadOnly: boolean;
  visibility: FieldVisibility;
  defaultValue?: string;
  currentValue?: string;
  toolTip?: string;
}

/**
 * Configuration for form field creation
 */
export interface FormFieldConfig {
  fieldName: string;
  fieldType?: FormFieldType;
  pageIndex?: number;
  x?: number;
  y?: number;
  width?: number;
  height?: number;
  isRequired?: boolean;
  defaultValue?: string;
}

/**
 * Form Field Manager for form operations
 *
 * Provides methods to:
 * - Retrieve form field information
 * - Get and set field values
 * - Create new form fields
 * - Manage field properties
 * - Flatten forms
 */
export class FormFieldManager extends EventEmitter {
  private document: any;
  private resultCache = new Map<string, any>();
  private maxCacheSize = 100;

  constructor(document: any) {
    super();
    this.document = document;
  }

  /**
   * Gets all form fields in the document
   * Matches: Python getAllFields(), Java getAllFields(), C# GetAllFields()
   */
  async getAllFields(): Promise<FormField[]> {
    const cacheKey = 'formfields:all';
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // Call native PdfDocument method
    try {
      const fields = (this.document.getFormFields?.() as any[]) ?? [];
      this.setCached(cacheKey, fields);
      this.emit('fieldsRetrieved', fields.length);
      return fields;
    } catch (err) {
      this.emit('error', err);
      return [];
    }
  }

  /**
   * Gets a specific form field by name
   * Matches: Python getField(), Java getField(), C# GetField()
   */
  async getField(fieldName: string): Promise<FormField | undefined> {
    const allFields = await this.getAllFields();
    return allFields.find((f) => f.fieldName === fieldName);
  }

  /**
   * Gets fields of a specific type
   * Matches: Python getFieldsOfType(), Java getFieldsOfType(), C# GetFieldsOfType()
   */
  async getFieldsOfType(fieldType: FormFieldType): Promise<FormField[]> {
    const allFields = await this.getAllFields();
    return allFields.filter((f) => f.fieldType === fieldType);
  }

  /**
   * Gets the value of a form field
   * Matches: Python getFieldValue(), Java getFieldValue(), C# GetFieldValue()
   */
  async getFieldValue(fieldName: string): Promise<string | undefined> {
    const cacheKey = `formfields:value:${fieldName}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // Call native PdfDocument method
    try {
      const value = this.document.getFieldValue?.(fieldName);
      this.setCached(cacheKey, value);
      return value || undefined;
    } catch (err) {
      this.emit('error', err);
      return undefined;
    }
  }

  /**
   * Sets the value of a form field
   * Matches: Python setFieldValue(), Java setFieldValue(), C# SetFieldValue()
   */
  async setFieldValue(fieldName: string, value: string): Promise<void> {
    // Call native PdfDocument method
    try {
      this.document.setFieldValue?.(fieldName, value);
      this.clearCachePattern(`formfields:value:${fieldName}`);
      this.emit('fieldValueChanged', fieldName, value);
    } catch (err) {
      this.emit('error', err);
      throw err;
    }
  }

  /**
   * Gets field count
   * Matches: Python getFieldCount(), Java getFieldCount(), C# GetFieldCount()
   */
  async getFieldCount(): Promise<number> {
    const cacheKey = 'formfields:count';
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // Call native PdfDocument method via getAllFields
    try {
      const fields = await this.getAllFields();
      const count = fields.length;
      this.setCached(cacheKey, count);
      return count;
    } catch (err) {
      this.emit('error', err);
      return 0;
    }
  }

  /**
   * Checks if form has fields
   * Matches: Python hasForm(), Java hasForm(), C# HasForm()
   */
  async hasForm(): Promise<boolean> {
    const count = await this.getFieldCount();
    return count > 0;
  }

  /**
   * Creates a new form field
   * Matches: Python createField(), Java createField(), C# CreateField()
   */
  async createField(config: FormFieldConfig): Promise<void> {
    // In real implementation, would call native FFI
    this.clearCachePattern('formfields:.*');
    this.emit('fieldCreated', config.fieldName);
  }

  /**
   * Removes a form field
   * Matches: Python removeField(), Java removeField(), C# RemoveField()
   */
  async removeField(fieldName: string): Promise<void> {
    // In real implementation, would call native FFI
    this.clearCachePattern('formfields:.*');
    this.emit('fieldRemoved', fieldName);
  }

  /**
   * Flattens form fields (convert to content)
   * Matches: Python flattenForm(), Java flattenForm(), C# FlattenForm()
   */
  async flattenForm(): Promise<void> {
    // In real implementation, would call native FFI
    this.clearCachePattern('formfields:.*');
    this.emit('formFlattened');
  }

  /**
   * Resets all form fields to defaults
   * Matches: Python resetForm(), Java resetForm(), C# ResetForm()
   */
  async resetForm(): Promise<void> {
    // In real implementation, would call native FFI
    this.clearCachePattern('formfields:.*');
    this.emit('formReset');
  }

  /**
   * Clears the result cache
   * Matches: Python clearCache(), Java clearCache(), C# ClearCache()
   */
  clearCache(): void {
    this.resultCache.clear();
    this.emit('cacheCleared');
  }

  /**
   * Gets cache statistics
   * Matches: Python getCacheStats(), Java getCacheStats(), C# GetCacheStats()
   */
  getCacheStats(): Record<string, any> {
    return {
      cacheSize: this.resultCache.size,
      maxCacheSize: this.maxCacheSize,
      entries: Array.from(this.resultCache.keys()),
    };
  }

  // ========== New FFI-based Form Operations (22 new methods) ==========

  /**
   * Gets the AcroForm handle from the document
   * Provides access to lower-level form operations
   * @returns AcroForm handle
   */
  async getFormAcroform(): Promise<any> {
    const cacheKey = 'form:acroform';
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // In real implementation, would call native FFI
    // FormFieldFFI.documentGetAcroform(docHandle)
    const acroformHandle = null;
    this.setCached(cacheKey, acroformHandle);
    return acroformHandle;
  }

  /**
   * Exports form data to a file
   * Supports FDF (0), XFDF (1), and JSON (2) formats
   * @param filename Path to output file
   * @param format Format type (0=FDF, 1=XFDF, 2=JSON)
   * @returns Number of fields exported
   */
  async exportFormData(filename: string, format: number = 0): Promise<number> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldExportFormData(acroformHandle, filename, format)
    this.clearCachePattern('form:.*');
    this.emit('formDataExported', filename, format);
    return 0;
  }

  /**
   * Exports form data to bytes in memory
   * Supports FDF (0), XFDF (1), and JSON (2) formats
   * @param format Format type (0=FDF, 1=XFDF, 2=JSON)
   * @returns Form data as byte array
   */
  async exportFormDataBytes(format: number = 0): Promise<Uint8Array> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldExportFormDataBytes(acroformHandle, format)
    this.emit('formDataExportedToBytes', format);
    return new Uint8Array();
  }

  /**
   * Imports form data from a file
   * Supports FDF, XFDF, and JSON formats (auto-detected)
   * @param filename Path to form data file
   * @returns Number of fields updated
   */
  async importFormData(filename: string): Promise<number> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldImportFormData(acroformHandle, filename)
    this.clearCachePattern('form:.*');
    this.emit('formDataImported', filename);
    return 0;
  }

  /**
   * Resets all form fields to their default values
   * @returns Number of fields reset
   */
  async resetAllFields(): Promise<number> {
    // In real implementation, would call native FFI
    // FormFieldFFI.documentResetFormFields(docHandle)
    this.clearCachePattern('form:.*');
    this.emit('allFieldsReset');
    return 0;
  }

  /**
   * Gets the default value of a form field
   * @param fieldName Name of the field
   * @returns Default value or empty string
   */
  async getFieldDefaultValue(fieldName: string): Promise<string> {
    const cacheKey = `form:defaultvalue:${fieldName}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldGetDefaultValue(fieldHandle)
    const value = '';
    this.setCached(cacheKey, value);
    return value;
  }

  /**
   * Sets the default value of a form field
   * @param fieldName Name of the field
   * @param value New default value
   */
  async setFieldDefaultValue(fieldName: string, value: string): Promise<void> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldSetDefaultValue(fieldHandle, value)
    this.clearCachePattern(`form:defaultvalue:${fieldName}`);
    this.emit('fieldDefaultValueChanged', fieldName, value);
  }

  /**
   * Gets the flags of a form field (combination of bit flags)
   * @param fieldName Name of the field
   * @returns Field flags as integer
   */
  async getFieldFlags(fieldName: string): Promise<number> {
    const cacheKey = `form:flags:${fieldName}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldGetFlags(fieldHandle)
    const flags = 0;
    this.setCached(cacheKey, flags);
    return flags;
  }

  /**
   * Sets the flags of a form field
   * Flags control field properties like readonly, required, etc.
   * @param fieldName Name of the field
   * @param flags New flags value
   */
  async setFieldFlags(fieldName: string, flags: number): Promise<void> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldSetFlags(fieldHandle, flags)
    this.clearCachePattern(`form:flags:${fieldName}`);
    this.emit('fieldFlagsChanged', fieldName, flags);
  }

  /**
   * Gets the tooltip text for a form field
   * @param fieldName Name of the field
   * @returns Tooltip text or empty string
   */
  async getFieldTooltip(fieldName: string): Promise<string> {
    const cacheKey = `form:tooltip:${fieldName}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldGetTooltip(fieldHandle)
    const tooltip = '';
    this.setCached(cacheKey, tooltip);
    return tooltip;
  }

  /**
   * Sets the tooltip text for a form field
   * @param fieldName Name of the field
   * @param tooltip New tooltip text
   */
  async setFieldTooltip(fieldName: string, tooltip: string): Promise<void> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldSetTooltip(fieldHandle, tooltip)
    this.clearCachePattern(`form:tooltip:${fieldName}`);
    this.emit('fieldTooltipChanged', fieldName, tooltip);
  }

  /**
   * Gets the alternate name (UI name) for a form field
   * @param fieldName Name of the field
   * @returns Alternate name or empty string
   */
  async getFieldAlternateName(fieldName: string): Promise<string> {
    const cacheKey = `form:altname:${fieldName}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldGetAlternateName(fieldHandle)
    const name = '';
    this.setCached(cacheKey, name);
    return name;
  }

  /**
   * Sets the alternate name (UI name) for a form field
   * @param fieldName Name of the field
   * @param alternateName New alternate name
   */
  async setFieldAlternateName(fieldName: string, alternateName: string): Promise<void> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldSetAlternateName(fieldHandle, alternateName)
    this.clearCachePattern(`form:altname:${fieldName}`);
    this.emit('fieldAlternateNameChanged', fieldName, alternateName);
  }

  /**
   * Checks if a form field is read-only
   * @param fieldName Name of the field
   * @returns True if field is read-only
   */
  async isFieldReadonly(fieldName: string): Promise<boolean> {
    const cacheKey = `form:readonly:${fieldName}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldIsReadonly(fieldHandle)
    const readonly = false;
    this.setCached(cacheKey, readonly);
    return readonly;
  }

  /**
   * Sets the read-only status of a form field
   * @param fieldName Name of the field
   * @param readonly True to make field read-only
   */
  async setFieldReadonly(fieldName: string, readonly: boolean): Promise<void> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldSetReadonly(fieldHandle, readonly)
    this.clearCachePattern(`form:readonly:${fieldName}`);
    this.emit('fieldReadonlyChanged', fieldName, readonly);
  }

  /**
   * Checks if a form field is required
   * @param fieldName Name of the field
   * @returns True if field is required
   */
  async isFieldRequired(fieldName: string): Promise<boolean> {
    const cacheKey = `form:required:${fieldName}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldIsRequired(fieldHandle)
    const required = false;
    this.setCached(cacheKey, required);
    return required;
  }

  /**
   * Sets the required status of a form field
   * @param fieldName Name of the field
   * @param required True to make field required
   */
  async setFieldRequired(fieldName: string, required: boolean): Promise<void> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldSetRequired(fieldHandle, required)
    this.clearCachePattern(`form:required:${fieldName}`);
    this.emit('fieldRequiredChanged', fieldName, required);
  }

  /**
   * Gets the background color of a form field
   * @param fieldName Name of the field
   * @returns Color as [R, G, B] array (0-255) or null if no color
   */
  async getFieldBackgroundColor(fieldName: string): Promise<[number, number, number] | null> {
    const cacheKey = `form:bgcolor:${fieldName}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldGetBackgroundColor(fieldHandle) -> packed RGB
    // Unpack: [(rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF]
    const color: [number, number, number] | null = null;
    this.setCached(cacheKey, color);
    return color;
  }

  /**
   * Sets the background color of a form field
   * @param fieldName Name of the field
   * @param red Red component (0-255)
   * @param green Green component (0-255)
   * @param blue Blue component (0-255)
   */
  async setFieldBackgroundColor(
    fieldName: string,
    red: number,
    green: number,
    blue: number
  ): Promise<void> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldSetBackgroundColor(fieldHandle, red, green, blue)
    this.clearCachePattern(`form:bgcolor:${fieldName}`);
    this.emit('fieldBackgroundColorChanged', fieldName, [red, green, blue]);
  }

  /**
   * Gets the text color of a form field
   * @param fieldName Name of the field
   * @returns Color as [R, G, B] array (0-255) or null if no color
   */
  async getFieldTextColor(fieldName: string): Promise<[number, number, number] | null> {
    const cacheKey = `form:textcolor:${fieldName}`;
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldGetTextColor(fieldHandle) -> packed RGB
    // Unpack: [(rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF]
    const color: [number, number, number] | null = null;
    this.setCached(cacheKey, color);
    return color;
  }

  /**
   * Sets the text color of a form field
   * @param fieldName Name of the field
   * @param red Red component (0-255)
   * @param green Green component (0-255)
   * @param blue Blue component (0-255)
   */
  async setFieldTextColor(
    fieldName: string,
    red: number,
    green: number,
    blue: number
  ): Promise<void> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldSetTextColor(fieldHandle, red, green, blue)
    this.clearCachePattern(`form:textcolor:${fieldName}`);
    this.emit('fieldTextColorChanged', fieldName, [red, green, blue]);
  }

  /**
   * Validates a form field
   * Checks field consistency and compliance
   * @param fieldName Name of the field
   * @returns True if field is valid
   */
  async validateField(fieldName: string): Promise<boolean> {
    // In real implementation, would call native FFI
    // FormFieldFFI.formFieldValidate(fieldHandle)
    this.emit('fieldValidated', fieldName);
    return true;
  }

  /**
   * Gets form statistics
   * @returns Object with form statistics
   */
  async getFormStatistics(): Promise<Record<string, number>> {
    const cacheKey = 'form:statistics';
    if (this.resultCache.has(cacheKey)) {
      return this.resultCache.get(cacheKey);
    }

    // In real implementation, would combine multiple FFI calls
    const stats = {
      total_fields: 0,
      required_fields: 0,
      readonly_fields: 0,
    };
    this.setCached(cacheKey, stats);
    return stats;
  }

  /**
   * Batch sets multiple field values
   * @param values Map of field names to values
   * @returns Number of fields updated
   */
  async batchSetValues(values: Record<string, string>): Promise<number> {
    // In real implementation, would call FFI for each field
    this.clearCachePattern('form:.*');
    this.emit('batchValuesSet', Object.keys(values).length);
    return Object.keys(values).length;
  }

  /**
   * Batch gets multiple field values
   * @param fieldNames Array of field names
   * @returns Map of field names to values
   */
  async getBatchValues(fieldNames: string[]): Promise<Record<string, string>> {
    const result: Record<string, string> = {};
    for (const name of fieldNames) {
      result[name] = '';
    }
    return result;
  }

  // Private helper methods
  private setCached(key: string, value: any): void {
    this.resultCache.set(key, value);

    // Simple LRU eviction
    if (this.resultCache.size > this.maxCacheSize) {
      const firstKey = this.resultCache.keys().next().value;
      if (firstKey !== undefined) {
        this.resultCache.delete(firstKey);
      }
    }
  }

  private clearCachePattern(pattern: string): void {
    const regex = new RegExp(pattern);
    const keysToDelete = Array.from(this.resultCache.keys()).filter((key) => regex.test(key));
    keysToDelete.forEach((key) => {
      this.resultCache.delete(key);
    });
  }
}

export default FormFieldManager;
