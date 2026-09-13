/**
 * PDF Oxide Node.js: Forms Management Example
 *
 * Demonstrates comprehensive PDF form management including:
 * - Creating and modifying form fields
 * - Setting field properties and styling
 * - Importing/exporting form data
 * - Working with AcroForm and field hierarchies
 * - Batch operations and form flattening
 */

import { PdfDocument, FormFieldManager, FormFieldType } from 'pdf_oxide';

async function main() {
  console.log('PDF Oxide - Form Management Examples\n');
  console.log('=====================================================\n');

  try {
    // Open PDF document with form
    const document = await PdfDocument.open('form.pdf');

    // Initialize Form Field Manager
    const formManager = new FormFieldManager(document);

    // Example 1: Form Field Access
    await formFieldAccess(formManager);

    // Example 2: Form Field Properties
    await formFieldProperties(formManager);

    // Example 3: Form Field Styling
    await formFieldStyling(formManager);

    // Example 4: Form Data Management
    await formDataManagement(formManager);

    // Example 5: Batch Form Operations
    await batchOperations(formManager);

    // Example 6: Field Validation
    await fieldValidation(formManager);

    // Example 7: Advanced Form Operations
    await advancedOperations(formManager);

    // Example 8: Form Statistics
    await formStatistics(formManager);

    document.close();
  } catch (error) {
    console.error('Error:', (error as Error).message);
    process.exit(1);
  }
}

/**
 * Example: Access and inspect form fields
 */
async function formFieldAccess(formManager: FormFieldManager): Promise<void> {
  console.log('=== Form Field Access ===\n');

  console.log('Get all fields:');
  console.log('  const allFields = await formManager.getAllFields();');
  console.log('  console.log("Total fields:", allFields.length);\n');

  console.log('Get fields on specific page:');
  console.log('  const page0Fields = await formManager.getAllFields();');
  console.log('  const pageFields = page0Fields.filter(f => f.pageIndex === 0);');
  console.log('  for (const field of pageFields) {');
  console.log('    console.log(field.fieldName + " on page 0");');
  console.log('  }\n');

  console.log('Get specific field info:');
  console.log('  const field = await formManager.getField("first_name");');
  console.log('  if (field) {');
  console.log('    console.log("Type:", field.fieldType);');
  console.log('    console.log("Position: (" + field.pageIndex + ")");');
  console.log('  }\n');
}

/**
 * Example: Manage form field properties
 */
async function formFieldProperties(formManager: FormFieldManager): Promise<void> {
  console.log('=== Form Field Properties ===\n');

  console.log('Get and set field values:');
  console.log('  const currentValue = await formManager.getFieldValue("name_field");');
  console.log('  if (currentValue) {');
  console.log('    console.log("Current:", currentValue);');
  console.log('  }');
  console.log('  await formManager.setFieldValue("name_field", "John Doe");\n');

  console.log('Manage field defaults:');
  console.log('  const defValue = await formManager.getFieldDefaultValue("name_field");');
  console.log('  await formManager.setFieldDefaultValue("name_field", "Enter name here");\n');

  console.log('Set field requirements:');
  console.log('  await formManager.setFieldRequired("email_field", true);');
  console.log('  const isRequired = await formManager.isFieldRequired("email_field");\n');

  console.log('Set field protection:');
  console.log('  await formManager.setFieldReadonly("id_field", true);');
  console.log('  const isReadonly = await formManager.isFieldReadonly("id_field");\n');
}

/**
 * Example: Style form fields
 */
async function formFieldStyling(formManager: FormFieldManager): Promise<void> {
  console.log('=== Form Field Styling ===\n');

  console.log('Set colors:');
  console.log('  // Yellow background');
  console.log('  await formManager.setFieldBackgroundColor("field1", 255, 255, 0);');
  console.log('  // Dark blue text');
  console.log('  await formManager.setFieldTextColor("field1", 0, 0, 128);\n');

  console.log('Get colors:');
  console.log('  const bgColor = await formManager.getFieldBackgroundColor("field1");');
  console.log('  if (bgColor) {');
  console.log('    console.log("Background RGB:", bgColor);');
  console.log('  }\n');

  console.log('Add tooltip and alternate name:');
  console.log('  await formManager.setFieldTooltip("field1", "Enter required information");');
  console.log('  await formManager.setFieldAlternateName("field1", "Full Name");\n');

  console.log('Get metadata:');
  console.log('  const tooltip = await formManager.getFieldTooltip("field1");');
  console.log('  const altName = await formManager.getFieldAlternateName("field1");\n');
}

/**
 * Example: Import and export form data
 */
async function formDataManagement(formManager: FormFieldManager): Promise<void> {
  console.log('=== Form Data Management ===\n');

  console.log('Export form data:');
  console.log('  // Export to FDF format (0)');
  console.log('  await formManager.exportFormData("form_data.fdf", 0);');
  console.log('  // Export to XFDF format (1)');
  console.log('  await formManager.exportFormData("form_data.xfdf", 1);');
  console.log('  // Export to JSON format (2)');
  console.log('  await formManager.exportFormData("form_data.json", 2);\n');

  console.log('Export to memory:');
  console.log('  const fdfBytes = await formManager.exportFormDataBytes(0);');
  console.log('  console.log("Exported " + fdfBytes.length + " bytes");\n');

  console.log('Import form data:');
  console.log('  const fieldsUpdated = await formManager.importFormData("form_data.fdf");');
  console.log('  console.log("Updated " + fieldsUpdated + " fields");\n');

  console.log('Reset fields to defaults:');
  console.log('  const resetCount = await formManager.resetAllFields();');
  console.log('  console.log("Reset " + resetCount + " fields");\n');
}

/**
 * Example: Batch form operations
 */
async function batchOperations(formManager: FormFieldManager): Promise<void> {
  console.log('=== Batch Form Operations ===\n');

  console.log('Set multiple field values:');
  console.log('  const values = {');
  console.log('    first_name: "John",');
  console.log('    last_name: "Doe",');
  console.log('    email: "john@example.com"');
  console.log('  };');
  console.log('  const updated = await formManager.batchSetValues(values);');
  console.log('  console.log("Updated " + updated + " fields");\n');

  console.log('Get multiple field values:');
  console.log('  const fieldNames = ["first_name", "last_name", "email"];');
  console.log('  const retrieved = await formManager.getBatchValues(fieldNames);');
  console.log('  Object.entries(retrieved).forEach(([name, value]) =>');
  console.log('    console.log(name + ": " + value)');
  console.log('  );\n');

  console.log('Form statistics:');
  console.log('  const stats = await formManager.getFormStatistics();');
  console.log('  console.log("Total fields:", stats.total_fields);');
  console.log('  console.log("Required:", stats.required_fields);');
  console.log('  console.log("Read-only:", stats.readonly_fields);\n');
}

/**
 * Example: Field validation and flags
 */
async function fieldValidation(formManager: FormFieldManager): Promise<void> {
  console.log('=== Field Validation and Flags ===\n');

  console.log('Validate text field:');
  console.log('  const isValid = await formManager.validateField("email_field");');
  console.log('  if (!isValid) {');
  console.log('    console.log("Invalid content");');
  console.log('  }\n');

  console.log('Manage field flags:');
  console.log('  const flags = await formManager.getFieldFlags("field1");');
  console.log('  // Set specific flags (bitwise OR)');
  console.log('  const newFlags = flags | 0x0002; // REQUIRED flag');
  console.log('  await formManager.setFieldFlags("field1", newFlags);\n');
}

/**
 * Example: Advanced form operations
 */
async function advancedOperations(formManager: FormFieldManager): Promise<void> {
  console.log('=== Advanced Form Operations ===\n');

  console.log('Get AcroForm handle:');
  console.log('  const acroform = await formManager.getFormAcroform();');
  console.log('  // Can be used for lower-level operations\n');

  console.log('Check form type:');
  console.log('  const hasAcroform = await formManager.getAllFields().then(f => f.length > 0);');
  console.log('  console.log("Has form:", hasAcroform);\n');

  console.log('Form field types:');
  console.log('  const fields = await formManager.getAllFields();');
  console.log('  const textFields = fields.filter(f => f.fieldType === FormFieldType.Text);');
  console.log('  const checkboxes = fields.filter(f => f.fieldType === FormFieldType.CheckBox);\n');
}

/**
 * Example: Form statistics and cache
 */
async function formStatistics(formManager: FormFieldManager): Promise<void> {
  console.log('=== Form Statistics and Cache ===\n');

  console.log('Form statistics:');
  console.log('  const stats = await formManager.getFormStatistics();');
  console.log('  console.log(JSON.stringify(stats, null, 2));\n');

  console.log('Cache statistics:');
  const cacheStats = formManager.getCacheStats();
  console.log(`  Current cache size: ${cacheStats.cacheSize} / ${cacheStats.maxCacheSize}`);
  console.log(`  Cached entries: ${cacheStats.entries.join(', ')}\n`);

  console.log('Clear cache:');
  console.log('  formManager.clearCache();\n');

  console.log('Event handling:');
  console.log('  formManager.on("fieldValueChanged", (fieldName, value) => {');
  console.log('    console.log("Field changed:", fieldName, value);');
  console.log('  });');
  console.log('  formManager.on("fieldReadonlyChanged", (fieldName, readonly) => {');
  console.log('    console.log("Readonly changed:", fieldName, readonly);');
  console.log('  });');
  console.log('  formManager.on("formDataImported", (filename) => {');
  console.log('    console.log("Form data imported from:", filename);');
  console.log('  });\n');
}

main().catch((error) => {
  console.error('Fatal error:', error);
  process.exit(1);
});
