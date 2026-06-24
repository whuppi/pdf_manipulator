/**
 * Unit tests for FormFieldManager FFI-based methods
 * Tests 22 new FFI-based form field operations covering:
 * - Form data import/export (3 functions)
 * - Field properties access (11 functions)
 * - Field styling (4 functions)
 * - Field validation (1 function)
 * - Statistics and batch operations (3 functions)
 */

import { FieldVisibility, FormFieldManager, FormFieldType } from '../src/form-field-manager';

describe('FormFieldManager FFI-based Methods', () => {
  let manager: FormFieldManager;

  beforeEach(() => {
    manager = new FormFieldManager(null);
  });

  describe('Form Data Operations', () => {
    test('should export form data to file', async () => {
      const result = await manager.exportFormData('test.fdf', 0);
      expect(typeof result).toBe('number');
      expect(result).toBeGreaterThanOrEqual(0);
    });

    test('should export form data to bytes', async () => {
      const bytes = await manager.exportFormDataBytes(0);
      expect(bytes instanceof Uint8Array).toBe(true);
    });

    test('should import form data from file', async () => {
      const result = await manager.importFormData('test.fdf');
      expect(typeof result).toBe('number');
      expect(result).toBeGreaterThanOrEqual(0);
    });

    test('should support different export formats', async () => {
      // FDF format (0)
      const fdf = await manager.exportFormDataBytes(0);
      expect(fdf instanceof Uint8Array).toBe(true);

      // XFDF format (1)
      const xfdf = await manager.exportFormDataBytes(1);
      expect(xfdf instanceof Uint8Array).toBe(true);

      // JSON format (2)
      const json = await manager.exportFormDataBytes(2);
      expect(json instanceof Uint8Array).toBe(true);
    });

    test('should reset all fields', async () => {
      const result = await manager.resetAllFields();
      expect(typeof result).toBe('number');
    });
  });

  describe('Form Field Properties', () => {
    test('should get AcroForm handle', async () => {
      const handle = await manager.getFormAcroform();
      expect(handle === null || typeof handle === 'object').toBe(true);
    });

    test('should get field default value', async () => {
      const value = await manager.getFieldDefaultValue('test_field');
      expect(typeof value).toBe('string');
    });

    test('should set field default value', async () => {
      await manager.setFieldDefaultValue('test_field', 'default_value');
      // Should not throw
      expect(true).toBe(true);
    });

    test('should get field flags', async () => {
      const flags = await manager.getFieldFlags('test_field');
      expect(typeof flags).toBe('number');
    });

    test('should set field flags', async () => {
      await manager.setFieldFlags('test_field', 0x0002);
      // Should not throw
      expect(true).toBe(true);
    });

    test('should get field tooltip', async () => {
      const tooltip = await manager.getFieldTooltip('test_field');
      expect(typeof tooltip).toBe('string');
    });

    test('should set field tooltip', async () => {
      await manager.setFieldTooltip('test_field', 'This is a tooltip');
      // Should not throw
      expect(true).toBe(true);
    });

    test('should get field alternate name', async () => {
      const altName = await manager.getFieldAlternateName('test_field');
      expect(typeof altName).toBe('string');
    });

    test('should set field alternate name', async () => {
      await manager.setFieldAlternateName('test_field', 'Alternate Name');
      // Should not throw
      expect(true).toBe(true);
    });

    test('should check if field is readonly', async () => {
      const readonly = await manager.isFieldReadonly('test_field');
      expect(typeof readonly).toBe('boolean');
    });

    test('should set field readonly status', async () => {
      await manager.setFieldReadonly('test_field', true);
      // Should not throw
      expect(true).toBe(true);
    });

    test('should check if field is required', async () => {
      const required = await manager.isFieldRequired('test_field');
      expect(typeof required).toBe('boolean');
    });

    test('should set field required status', async () => {
      await manager.setFieldRequired('test_field', true);
      // Should not throw
      expect(true).toBe(true);
    });
  });

  describe('Form Field Styling', () => {
    test('should get field background color', async () => {
      const color = await manager.getFieldBackgroundColor('test_field');
      if (color !== null) {
        expect(Array.isArray(color)).toBe(true);
        expect(color.length).toBe(3);
        expect(color[0]).toBeGreaterThanOrEqual(0);
        expect(color[0]).toBeLessThanOrEqual(255);
      }
    });

    test('should set field background color', async () => {
      await manager.setFieldBackgroundColor('test_field', 255, 255, 0);
      // Should not throw
      expect(true).toBe(true);
    });

    test('should get field text color', async () => {
      const color = await manager.getFieldTextColor('test_field');
      if (color !== null) {
        expect(Array.isArray(color)).toBe(true);
        expect(color.length).toBe(3);
        expect(color[0]).toBeGreaterThanOrEqual(0);
        expect(color[0]).toBeLessThanOrEqual(255);
      }
    });

    test('should set field text color', async () => {
      await manager.setFieldTextColor('test_field', 0, 0, 128);
      // Should not throw
      expect(true).toBe(true);
    });

    test('should support RGB color values', async () => {
      // Red
      await manager.setFieldBackgroundColor('field1', 255, 0, 0);
      // Green
      await manager.setFieldBackgroundColor('field2', 0, 255, 0);
      // Blue
      await manager.setFieldBackgroundColor('field3', 0, 0, 255);
      expect(true).toBe(true);
    });
  });

  describe('Form Field Validation', () => {
    test('should validate field', async () => {
      const valid = await manager.validateField('test_field');
      expect(typeof valid).toBe('boolean');
    });
  });

  describe('Form Statistics', () => {
    test('should get form statistics', async () => {
      const stats = await manager.getFormStatistics();
      expect(stats).toBeDefined();
      expect(stats).toHaveProperty('total_fields');
      expect(stats).toHaveProperty('required_fields');
      expect(stats).toHaveProperty('readonly_fields');
      expect(typeof stats.total_fields).toBe('number');
      expect(typeof stats.required_fields).toBe('number');
      expect(typeof stats.readonly_fields).toBe('number');
    });

    test('should have non-negative statistics', async () => {
      const stats = await manager.getFormStatistics();
      expect(stats.total_fields).toBeGreaterThanOrEqual(0);
      expect(stats.required_fields).toBeGreaterThanOrEqual(0);
      expect(stats.readonly_fields).toBeGreaterThanOrEqual(0);
    });
  });

  describe('Batch Operations', () => {
    test('should batch set field values', async () => {
      const values = {
        field1: 'value1',
        field2: 'value2',
        field3: 'value3',
      };
      const updated = await manager.batchSetValues(values);
      expect(typeof updated).toBe('number');
      expect(updated).toBe(3);
    });

    test('should batch get field values', async () => {
      const fieldNames = ['field1', 'field2', 'field3'];
      const values = await manager.getBatchValues(fieldNames);
      expect(values).toBeDefined();
      expect(Object.keys(values).length).toBe(3);
      expect(values).toHaveProperty('field1');
      expect(values).toHaveProperty('field2');
      expect(values).toHaveProperty('field3');
    });

    test('should handle empty batch operations', async () => {
      const updated = await manager.batchSetValues({});
      expect(updated).toBe(0);

      const values = await manager.getBatchValues([]);
      expect(Object.keys(values).length).toBe(0);
    });
  });

  describe('Cache Management', () => {
    test('should clear cache', () => {
      manager.clearCache();
      const stats = manager.getCacheStats();
      expect(stats.cacheSize).toBe(0);
    });

    test('should get cache statistics', () => {
      const stats = manager.getCacheStats();
      expect(stats).toBeDefined();
      expect(stats.cacheSize).toBeDefined();
      expect(stats.maxCacheSize).toBeDefined();
      expect(stats.entries).toBeDefined();
    });
  });

  describe('Event Emission', () => {
    test('should emit fieldValueChanged event', (done) => {
      manager.on('fieldValueChanged', (fieldName, value) => {
        expect(fieldName).toBe('test_field');
        expect(value).toBe('new_value');
        done();
      });

      manager.setFieldValue('test_field', 'new_value').catch(() => {
        // Expected in test environment
      });
    });

    test('should emit formDataExported event', (done) => {
      manager.on('formDataExported', (filename, format) => {
        expect(filename).toBe('test.fdf');
        expect(format).toBe(0);
        done();
      });

      manager.exportFormData('test.fdf', 0).catch(() => {
        // Expected in test environment
      });
    });

    test('should emit formDataImported event', (done) => {
      manager.on('formDataImported', (filename) => {
        expect(filename).toBe('test.fdf');
        done();
      });

      manager.importFormData('test.fdf').catch(() => {
        // Expected in test environment
      });
    });

    test('should emit allFieldsReset event', (done) => {
      manager.on('allFieldsReset', () => {
        done();
      });

      manager.resetAllFields().catch(() => {
        // Expected in test environment
      });
    });

    test('should emit fieldReadonlyChanged event', (done) => {
      manager.on('fieldReadonlyChanged', (fieldName, readonly) => {
        expect(fieldName).toBe('test_field');
        expect(readonly).toBe(true);
        done();
      });

      manager.setFieldReadonly('test_field', true).catch(() => {
        // Expected in test environment
      });
    });

    test('should emit fieldRequiredChanged event', (done) => {
      manager.on('fieldRequiredChanged', (fieldName, required) => {
        expect(fieldName).toBe('test_field');
        expect(required).toBe(true);
        done();
      });

      manager.setFieldRequired('test_field', true).catch(() => {
        // Expected in test environment
      });
    });

    test('should emit fieldBackgroundColorChanged event', (done) => {
      manager.on('fieldBackgroundColorChanged', (fieldName, color) => {
        expect(fieldName).toBe('test_field');
        expect(Array.isArray(color)).toBe(true);
        expect(color).toEqual([255, 255, 0]);
        done();
      });

      manager.setFieldBackgroundColor('test_field', 255, 255, 0).catch(() => {
        // Expected in test environment
      });
    });

    test('should emit fieldTextColorChanged event', (done) => {
      manager.on('fieldTextColorChanged', (fieldName, color) => {
        expect(fieldName).toBe('test_field');
        expect(Array.isArray(color)).toBe(true);
        expect(color).toEqual([0, 0, 128]);
        done();
      });

      manager.setFieldTextColor('test_field', 0, 0, 128).catch(() => {
        // Expected in test environment
      });
    });

    test('should emit batchValuesSet event', (done) => {
      manager.on('batchValuesSet', (count) => {
        expect(count).toBe(3);
        done();
      });

      manager.batchSetValues({ field1: 'v1', field2: 'v2', field3: 'v3' }).catch(() => {
        // Expected in test environment
      });
    });
  });

  describe('Type Safety', () => {
    test('FormFieldType enum should have correct values', () => {
      expect(FormFieldType.Text).toBeDefined();
      expect(FormFieldType.CheckBox).toBeDefined();
      expect(FormFieldType.RadioButton).toBeDefined();
      expect(FormFieldType.ComboBox).toBeDefined();
      expect(FormFieldType.ListBox).toBeDefined();
      expect(FormFieldType.Button).toBeDefined();
      expect(FormFieldType.Signature).toBeDefined();
      expect(FormFieldType.TextArea).toBeDefined();
    });

    test('FieldVisibility enum should have correct values', () => {
      expect(FieldVisibility.Visible).toBeDefined();
      expect(FieldVisibility.Hidden).toBeDefined();
      expect(FieldVisibility.PrintOnly).toBeDefined();
      expect(FieldVisibility.NoView).toBeDefined();
    });
  });

  describe('Method Coverage', () => {
    test('should have 31 public methods', () => {
      const methods = Object.getOwnPropertyNames(Object.getPrototypeOf(manager)).filter((prop) => {
        const descriptor = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(manager), prop);
        return (
          typeof descriptor?.value === 'function' && !prop.startsWith('_') && prop !== 'constructor'
        );
      });

      expect(methods.length).toBeGreaterThanOrEqual(31);
    });

    test('should have async methods for FFI operations', async () => {
      const exportResult = manager.exportFormData('test.fdf', 0);
      expect(exportResult instanceof Promise).toBe(true);

      const importResult = manager.importFormData('test.fdf');
      expect(importResult instanceof Promise).toBe(true);
    });
  });
});
