import { beforeEach, describe, expect, it } from 'vitest';
import {
  FieldVisibility,
  FormFieldManager,
  FormFieldState,
  FormFieldType,
} from '../src/form-field-manager';
import type { PdfDocument } from '../src/pdf-document';

describe('FormFieldManager', () => {
  let mockDocument: PdfDocument;
  let manager: FormFieldManager;

  beforeEach(() => {
    mockDocument = {
      filePath: 'test.pdf',
      pageCount: 10,
    } as PdfDocument;
    manager = new FormFieldManager(mockDocument);
  });

  describe('Initialization', () => {
    it('should create manager successfully', () => {
      expect(manager).toBeDefined();
    });

    it('should reject null document', () => {
      expect(() => new FormFieldManager(null)).toThrow();
    });

    it('should reject undefined document', () => {
      expect(() => new FormFieldManager(undefined)).toThrow();
    });
  });

  describe('FormFieldType enum', () => {
    it('should have all field types', () => {
      expect(FormFieldType.TEXT).toBe('text');
      expect(FormFieldType.CHECKBOX).toBe('checkbox');
      expect(FormFieldType.RADIO_BUTTON).toBe('radio');
      expect(FormFieldType.LIST_BOX).toBe('list_box');
      expect(FormFieldType.COMBO_BOX).toBe('combo_box');
      expect(FormFieldType.BUTTON).toBe('button');
      expect(FormFieldType.SIGNATURE).toBe('signature');
      expect(FormFieldType.FILE_CHOOSER).toBe('file_chooser');
    });
  });

  describe('FieldVisibility enum', () => {
    it('should have all visibility states', () => {
      expect(FieldVisibility.VISIBLE).toBe('visible');
      expect(FieldVisibility.HIDDEN).toBe('hidden');
      expect(FieldVisibility.PRINT_ONLY).toBe('print_only');
      expect(FieldVisibility.NO_VIEW).toBe('no_view');
    });
  });

  describe('FormFieldState enum', () => {
    it('should have all states', () => {
      expect(FormFieldState.NORMAL).toBe('normal');
      expect(FormFieldState.READONLY).toBe('readonly');
      expect(FormFieldState.REQUIRED).toBe('required');
      expect(FormFieldState.DISABLED).toBe('disabled');
    });
  });

  describe('Create Field', () => {
    it('should create field with default config', async () => {
      const result = await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'username',
        },
        50,
        100
      );

      expect(result).toBeDefined();
      expect(result.fieldName).toBe('username');
      expect(result.fieldType).toBe(FormFieldType.TEXT);
      expect(result.pageIndex).toBe(0);
    });

    it('should create field with custom config', async () => {
      const result = await manager.createField(
        0,
        {
          fieldType: FormFieldType.CHECKBOX,
          fieldName: 'agree',
          fieldValue: 'on',
          state: FormFieldState.REQUIRED,
        },
        50,
        100,
        30,
        20
      );

      expect(result.fieldType).toBe(FormFieldType.CHECKBOX);
      expect(result.currentValue).toBe('on');
      expect(result.isRequired).toBe(true);
      expect(result.width).toBe(30);
      expect(result.height).toBe(20);
    });

    it('should reject invalid page index', async () => {
      await expect(
        manager.createField(
          10,
          {
            fieldType: FormFieldType.TEXT,
            fieldName: 'test',
          },
          50,
          100
        )
      ).rejects.toThrow();
    });

    it('should reject invalid width', async () => {
      await expect(
        manager.createField(
          0,
          {
            fieldType: FormFieldType.TEXT,
            fieldName: 'test',
          },
          50,
          100,
          5
        )
      ).rejects.toThrow();
    });

    it('should reject invalid height', async () => {
      await expect(
        manager.createField(
          0,
          {
            fieldType: FormFieldType.TEXT,
            fieldName: 'test',
          },
          50,
          100,
          100,
          2000
        )
      ).rejects.toThrow();
    });

    it('should reject negative position', async () => {
      await expect(
        manager.createField(
          0,
          {
            fieldType: FormFieldType.TEXT,
            fieldName: 'test',
          },
          -10,
          100
        )
      ).rejects.toThrow();
    });
  });

  describe('Set Field Value', () => {
    it('should set field value successfully', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'email',
        },
        50,
        100
      );

      const result = await manager.setFieldValue('email', 'test@example.com');
      expect(result).toBe(true);
    });

    it('should reject nonexistent field', async () => {
      await expect(manager.setFieldValue('nonexistent', 'value')).rejects.toThrow();
    });

    it('should reject readonly field', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'readonly',
          state: FormFieldState.READONLY,
        },
        50,
        100
      );

      await expect(manager.setFieldValue('readonly', 'new_value')).rejects.toThrow();
    });

    it('should reject disabled field', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'disabled',
          state: FormFieldState.DISABLED,
        },
        50,
        100
      );

      await expect(manager.setFieldValue('disabled', 'new_value')).rejects.toThrow();
    });
  });

  describe('Get Field Value', () => {
    it('should get field value after set', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'name',
          fieldValue: 'John',
        },
        50,
        100
      );

      const value = await manager.getFieldValue('name');
      expect(value).toBe('John');
    });

    it('should reject nonexistent field', async () => {
      await expect(manager.getFieldValue('nonexistent')).rejects.toThrow();
    });

    it('should return undefined for unset field', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'optional',
        },
        50,
        100
      );

      const value = await manager.getFieldValue('optional');
      expect(value).toBeUndefined();
    });
  });

  describe('Get Field Info', () => {
    it('should get field information', async () => {
      const created = await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'address',
        },
        50,
        100,
        200,
        30
      );

      const info = await manager.getFieldInfo('address');
      expect(info.fieldName).toBe('address');
      expect(info.fieldType).toBe(FormFieldType.TEXT);
      expect(info.pageIndex).toBe(0);
    });

    it('should reject nonexistent field', async () => {
      await expect(manager.getFieldInfo('nonexistent')).rejects.toThrow();
    });
  });

  describe('Get All Fields', () => {
    it('should return empty list for empty form', async () => {
      const fields = await manager.getAllFields();
      expect(fields).toHaveLength(0);
    });

    it('should return multiple fields', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'first_name',
        },
        50,
        100
      );
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'last_name',
        },
        50,
        130
      );

      const fields = await manager.getAllFields();
      expect(fields).toHaveLength(2);
    });
  });

  describe('Get Page Fields', () => {
    it('should return fields on specific page', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'page0_field',
        },
        50,
        100
      );
      await manager.createField(
        1,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'page1_field',
        },
        50,
        100
      );

      const page0Fields = await manager.getPageFields(0);
      expect(page0Fields).toHaveLength(1);
      expect(page0Fields[0].fieldName).toBe('page0_field');
    });

    it('should reject invalid page', async () => {
      await expect(manager.getPageFields(10)).rejects.toThrow();
    });
  });

  describe('Update Field Config', () => {
    it('should update field configuration', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'field1',
        },
        50,
        100
      );

      const updated = await manager.updateFieldConfig('field1', {
        fieldType: FormFieldType.TEXT,
        fieldName: 'field1',
        fieldValue: 'updated',
        state: FormFieldState.READONLY,
      });

      expect(updated.currentValue).toBe('updated');
      expect(updated.state).toBe(FormFieldState.READONLY);
    });

    it('should reject nonexistent field', async () => {
      await expect(
        manager.updateFieldConfig('nonexistent', {
          fieldType: FormFieldType.TEXT,
          fieldName: 'nonexistent',
        })
      ).rejects.toThrow();
    });
  });

  describe('Delete Field', () => {
    it('should delete field', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'temp_field',
        },
        50,
        100
      );

      const result = await manager.deleteField('temp_field');
      expect(result).toBe(true);

      await expect(manager.getFieldValue('temp_field')).rejects.toThrow();
    });

    it('should reject nonexistent field', async () => {
      await expect(manager.deleteField('nonexistent')).rejects.toThrow();
    });
  });

  describe('Batch Operations', () => {
    it('should batch create fields', async () => {
      const fields = await manager.batchCreateFields(0, [
        {
          config: {
            fieldType: FormFieldType.TEXT,
            fieldName: 'field1',
          },
          x: 50,
          y: 100,
          width: 150,
          height: 20,
        },
        {
          config: {
            fieldType: FormFieldType.CHECKBOX,
            fieldName: 'field2',
          },
          x: 50,
          y: 130,
          width: 20,
          height: 20,
        },
      ]);

      expect(fields).toHaveLength(2);
      expect(fields[0].fieldName).toBe('field1');
      expect(fields[1].fieldName).toBe('field2');
    });

    it('should batch set values', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'name',
        },
        50,
        100
      );
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'email',
        },
        50,
        130
      );

      const values = new Map([
        ['name', 'John Doe'],
        ['email', 'john@example.com'],
      ]);

      const updated = await manager.batchSetValues(values);
      expect(updated).toBe(2);
    });

    it('should get batch values', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'name',
          fieldValue: 'John',
        },
        50,
        100
      );
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'city',
          fieldValue: 'NYC',
        },
        50,
        130
      );

      const values = await manager.getBatchValues(['name', 'city']);

      expect(values.get('name')).toBe('John');
      expect(values.get('city')).toBe('NYC');
    });
  });

  describe('Form Statistics', () => {
    it('should get empty statistics', () => {
      const stats = manager.getFormStatistics();

      expect(stats.total_fields).toBe(0);
      expect(stats.page_count).toBe(10);
      expect(Object.keys(stats.field_types)).toHaveLength(0);
    });

    it('should track field statistics', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'field1',
          state: FormFieldState.REQUIRED,
        },
        50,
        100
      );
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.CHECKBOX,
          fieldName: 'field2',
          state: FormFieldState.READONLY,
        },
        50,
        130
      );

      const stats = manager.getFormStatistics();

      expect(stats.total_fields).toBe(2);
      expect(stats.required_fields).toBe(1);
      expect(stats.readonly_fields).toBe(1);
      expect(stats.field_types['text']).toBe(1);
      expect(stats.field_types['checkbox']).toBe(1);
    });
  });

  describe('Cache Management', () => {
    it('should clear cache', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'field1',
        },
        50,
        100
      );

      let fields = await manager.getAllFields();
      expect(fields).toHaveLength(1);

      manager.clearCache();

      fields = await manager.getAllFields();
      expect(fields).toHaveLength(0);
    });

    it('should clear via clearFieldCache', async () => {
      await manager.createField(
        0,
        {
          fieldType: FormFieldType.TEXT,
          fieldName: 'field1',
        },
        50,
        100
      );

      manager.clearFieldCache();

      const fields = await manager.getAllFields();
      expect(fields).toHaveLength(0);
    });
  });
});
