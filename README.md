# pdf_manipulator (v0 — deprecated)

> **This branch is archived.** The v0 Android-only version based on iText7 is no longer maintained.
>
> **The package has been completely rewritten** — cross-platform (iOS, Android, macOS, Windows, Linux, Web), powered by a Rust engine, with a new API.
>
> **Switch to the new version:** [`dev` branch](https://github.com/whuppi/pdf_manipulator/tree/dev) · [Migration guide](https://github.com/whuppi/pdf_manipulator/blob/dev/docs/MIGRATION.md) · [pub.dev](https://pub.dev/packages/pdf_manipulator)

---

### What was this?

The original Android-only Flutter plugin for PDF manipulation (merge, split, compress, watermark, encrypt, etc.) using iText7. Android 5.0+ only. AGPL-3.0 licensed due to iText7.

## Package description

A flutter plugin for doing various manipulations such as merge, split, compress and many more on PDF easily.

**Note:** This project utilises itext7 for various operations involving PDFs and since itext7 AGPL V3 License is used in this plugin, it is also licenced under this licence. The project/plugin developer, the owner of the copyright, and the contributors are not accountable or liable for any damage resulting from this project/plugin.

## Features

- Works on Android 5.0 (API level 21) or later.
- Works with both absolute file path and Android native Uri.
- Supports merging multiple PDFs.
- Supports splitting PDF.
- Supports rotating PDF pages.
- Supports deleting PDF pages.
- Supports reordering PDF pages.
- Supports rotating, deleting, reordering PDF pages at the same time for more efficiency.
- Supports compressing PDF.
- Supports watermarking PDF.
- Supports encrypting PDF.
- Supports decrypting PDF.
- Supports converting images to PDF.
- Supports getting PDF validity and protection info.
- Supports getting PDF page size info.

**Note:** To use it in realease mode you will need to create a file named proguard-rules.pro in your project Android->App->proguard-rules.pro. In that file you need to add the below block of text at the end of the file.
```
# To use iText in release mode Otherwise we get
# PlatformException AbstractITextEvent is only for internal usage.
-keep public class com.itextpdf.**
-keep public class org.apache.**
```
For reference see this [project file](https://github.com/chaudharydeepanshu/files_tools/blob/main/android/app/proguard-rules.pro).

Then, setup your App->build.gradle buildTypes release config as done in this [project file](https://github.com/chaudharydeepanshu/files_tools/blob/54774ecf28d37c2a27d4fc666f7b178ad84ac46b/android/app/build.gradle#L77).
If you don't do all of this then you will get PlatformException: AbstractITextEvent is only for internal usage in Release build.

**Note:** If you are getting errors in you IDE after updating this plugin to newer version and the error contains works like Redeclaration, Conflicting declarations, Overload resolution ambiguity then to fix that you probably need to remove the older version of plugin from pub cache `C:\Users\username\AppData\Local\Pub\Cache\hosted\pub.dev\older_version` or simply run `flutter clean`.

## Getting started

- In pubspec.yaml, add this dependency:

```yaml
pdf_manipulator: 
```

- Add this package to your project:

```dart
import 'package:pdf_manipulator/pdf_manipulator.dart';
```

## Basic Usage

### Merging multiple PDFs

```dart
String? mergedPdfPath = await PdfManipulator().mergePDFs(
  params: PDFMergerParams(pdfsPaths: [pdfPath1, padfPath2]),
);
```

### Spliting PDF

```dart
String? mergedPdfPath = await PdfManipulator().mergePDFs(
  params: PDFMergerParams(pdfsPaths: [pdfPath1, padfPath2]),
);
```

#### Split PDF by page count

```dart
List<String>? splitPdfPaths = await PdfManipulator().splitPDF(
  params: PDFSplitterParams(pdfPath: pdfPath, pageCount: 2),
);
```

#### Split PDF by size

```dart
List<String>? splitPdfPaths = await PdfManipulator().splitPDF(
  params: PDFSplitterParams(pdfPath: pdfPath, byteSize: splitSize),
);
```

#### Split PDF by page numbers

```dart
List<String>? splitPdfPaths = await PdfManipulator().splitPDF(
  params: PDFSplitterParams(pdfPath: pdfPath, pageNumbers: [2, 5]),
);
```

#### Extract PDF pages by page range

```dart
List<String>? splitPdfPaths = await PdfManipulator().splitPDF(
  params: PDFSplitterParams(pdfPath: pdfPath, pageRanges: ["2", "5-10"]),
);
```

### Rotaing PDF pages

```dart
String? rotatedPagesPdfPath = await PdfManipulator().pdfPageRotator(
  params: PDFPageRotatorParams(pdfPath: pdfPath, pagesRotationInfo: [PageRotationInfo(pageNumber: 1, rotationAngle: 180)]),
);
```

### Deleting PDF pages

```dart
String? deletedPagesPdfPath = await PdfManipulator().pdfPageDeleter(
  params: PDFPageDeleterParams(pdfPath: pdfPath, pageNumbers: [1, 2, 3]),
);
```

### Reordering PDF pages

```dart
String? reorderedPagesPdfPath = await PdfManipulator().pdfPageReorder(
  params: PDFPageReorderParams(pdfPath: pdfPath, pageNumbers: [4, 1]),
);
```

### Rotating, Deleting, Reordering PDF pages at the same time

```dart
String? rotatedDeletedReorderedPagesPdfPath = await PdfManipulator().pdfPageRotatorDeleterReorder(
  params: PDFPageRotatorDeleterReorderParams(
      pdfPath: pdfPath,
      pagesRotationInfo: [PageRotationInfo(pageNumber: 1, rotationAngle: 180)],
      pageNumbersForReorder: [4, 3, 2, 1],
      pageNumbersForDeleter: [3, 2]),
);
```

### Compressing PDF

```dart
String? compressedPdfPath = await PdfManipulator().pdfCompressor(
  params: PDFCompressorParams(pdfPath: pdfPath, imageQuality: 100, imageScale: 1),
);
```

### Watermarking PDF

```dart
String? watermarkedPdfPath = await PdfManipulator().pdfWatermark(
  params: PDFWatermarkParams(
      pdfPath: pdfPath,
      text: "Watermark Text",
      watermarkColor: Colors.red,
      fontSize: 50,
      watermarkLayer: WatermarkLayer.overContent,
      opacity: 0.7,
      positionType: PositionType.center),
);
```

**Note:** When using `PositionType.custom` you need to provide `customPositionXCoordinatesList` and `customPositionYCoordinatesList`.

### Encrypting PDF

```dart
String? encryptedPdfPath = await PdfManipulator().pdfEncryption(
  params: PDFEncryptionParams(
      pdfPath: pdfPath,
      ownerPassword: "ownerpw",
      userPassword: "userpw",
      encryptionAES256: true // Set true to enable encryptionAES256 encryption.
);
```

`PDFEncryptionParams` other parameters with their default values is as follows:-
- `bool allowPrinting = false` Set true to allow printing permission.
- `bool allowModifyContents = false` Set true to allow modify permission.
- `bool allowCopy = false` Set true to allow copy permission.
- `bool allowModifyAnnotations = false` Set true to allow modifying annotations permission.
- `bool allowFillIn = false` Set true to allow fill in permission.
- `bool allowScreenReaders = false` Set true to allow screen readers permission.
- `bool allowAssembly = false` Set true to allow assembly permission.
- `bool allowDegradedPrinting = false` Set true to allow degraded printing permission.
- `bool standardEncryptionAES40 = false` Set true to enable StandardEncryptionAES40 encryption. standardEncryptionAES40 implicitly sets doNotEncryptMetadata and encryptEmbeddedFilesOnly as false.
- `bool standardEncryptionAES128 = false` Set true to enable StandardEncryptionAES128 encryption. standardEncryptionAES128 implicitly sets EncryptionConstants.EMBEDDED_FILES_ONLY as false.
- `bool encryptionAES128 = false` Set true to enable encryptionAES128 encryption.
- `bool encryptEmbeddedFilesOnly = false` Set true to encrypt embedded files only.
- `bool doNotEncryptMetadata = false` Set true to not encrypt metadata.

**Note:** Please be aware that the passed encryption types may override permissions.

### Decrypting PDF

```dart
String? decryptedPdfPath = await PdfManipulator().pdfDecryption(
  params: PDFDecryptionParams(
      pdfPath: pdfPath,
      password: ownerOrUserPassword,
);
```

### Converting images to PDF

```dart
List<String>? pdfsPaths = await PdfManipulator().imagesToPdfs(
  params: ImagesToPDFsParams(
      imagesPaths: imagesPaths,
      createSinglePdf: false,
);
```

Images in JPEG, JPEG2000, GIF, PNG, BMP, WMF, TIFF, CCITT and JBIG2 formats are supported.

### PDF validity and protection info

```dart
PdfValidityAndProtection? pdfValidityAndProtectionInfo = await PdfManipulator().pdfValidityAndProtection(
  params: PDFValidityAndProtectionParams(
      pdfPath: pdfPath,
);

/// Getting info.
bool? isPDFValid = pdfValidityAndProtectionInfo?.isPDFValid;
bool? isOwnerPasswordProtected = pdfValidityAndProtectionInfo?.isOwnerPasswordProtected;
bool? isOpenPasswordProtected = pdfValidityAndProtectionInfo?.isOpenPasswordProtected;
bool? isPrintingAllowed = pdfValidityAndProtectionInfo?.isPrintingAllowed;
bool? isModifyContentsAllowed = pdfValidityAndProtectionInfo?.isModifyContentsAllowed;
```
Don't provide password if you just want to check validity and protection. Only provide password if you want to check if that password is correct or not.

**Note:** If you only want to check validity and protection then I suggest to use [pdf_bitmaps](https://pub.dev/packages/pdf_bitmaps) as that is fast and requires less memory.

### PDF page size info

```dart
List<PageSizeInfo>? pdfPagesSizeInfo = await PdfManipulator().pdfPagesSize(
  params: PDFPagesSizeParams(
      pdfPath: pdfPath,
);

/// Getting 1st page info.
double? widthOfPage = pdfPagesSizeInfo[0]?.widthOfPage;
double? heightOfPage = pdfPagesSizeInfo[0]?.heightOfPage;
```

**Note:** If you only want to get page size then I suggest to use [pdf_bitmaps](https://pub.dev/packages/pdf_bitmaps) as that is fast and requires less memory.


Note: To try the demos shown in below images run the example included in this plugin.

<img src="https://user-images.githubusercontent.com/85361211/201522048-1f0c9cd3-e25e-4304-bae9-3673097bfbf1.jpeg" width="20%"></img> <img src="https://user-images.githubusercontent.com/85361211/201522051-db1595b6-d229-4e46-a765-1d09728a5f8c.jpeg" width="20%"></img> <img src="https://user-images.githubusercontent.com/85361211/201522053-7fbf7531-9264-4831-97aa-6d068ee18c4a.jpeg" width="20%"></img> <img src="https://user-images.githubusercontent.com/85361211/201522055-161fd60b-b656-4db3-b310-0b2c1dcb7e67.jpeg" width="20%"></img>
