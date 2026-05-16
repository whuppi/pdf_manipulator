import 'dart:typed_data';

import 'package:pdf_manipulator/src/core/pdf_rect.dart';
import 'package:pdf_manipulator/src/platform/pdf_platform.dart';

/// A mutable PDF editing session — parse once, mutate N times, save once.
///
/// Created via `pdf.openEditor(bytes)` where `pdf` is a [Pdf] instance.
///
/// ```dart
/// final pdf = Pdf();
/// final editor = await pdf.openEditor(bytes);
/// await editor.setTitle('Updated');
/// await editor.rotatePage(0, degrees: 90);
/// final result = await editor.save();
/// await editor.dispose();
/// ```
class PdfEditor {
  final PdfEditorHandle _handle;
  PdfEditor(this._handle);

  Future<int> get pageCount => _handle.pageCount;
  Future<String> get version => _handle.version;
  Future<bool> get isModified => _handle.isModified;

  Future<String> getTitle() => _handle.getTitle();
  Future<void> setTitle(String value) => _handle.setTitle(value);
  Future<String> getAuthor() => _handle.getAuthor();
  Future<void> setAuthor(String value) => _handle.setAuthor(value);
  Future<String> getSubject() => _handle.getSubject();
  Future<void> setSubject(String value) => _handle.setSubject(value);
  Future<String> getKeywords() => _handle.getKeywords();
  Future<void> setKeywords(String value) => _handle.setKeywords(value);

  Future<void> rotatePage(int pageIndex, {required int degrees}) =>
      _handle.rotatePage(pageIndex, degrees: degrees);
  Future<void> rotateAllPages({required int degrees}) =>
      _handle.rotateAllPages(degrees: degrees);
  Future<PdfRect> getPageMediaBox(int pageIndex) =>
      _handle.getPageMediaBox(pageIndex);
  Future<void> deletePage(int pageIndex) => _handle.deletePage(pageIndex);
  Future<void> movePage({required int from, required int to}) =>
      _handle.movePage(from: from, to: to);
  Future<Uint8List> extractPages(List<int> pages) =>
      _handle.extractPages(pages);
  Future<void> mergeFrom(Uint8List otherPdf) => _handle.mergeFrom(otherPdf);

  Future<int> optimizeImages({int quality = 75}) =>
      _handle.optimizeImages(quality: quality);
  Future<int> unembedStandardFonts() => _handle.unembedStandardFonts();
  Future<void> addWatermark(int pageIndex, String text,
      {double fontSize = 48, double rotation = 45, double opacity = 0.3,
       double r = 0.5, double g = 0.5, double b = 0.5}) =>
      _handle.addWatermark(pageIndex, text,
          fontSize: fontSize, rotation: rotation, opacity: opacity,
          r: r, g: g, b: b);

  Future<void> embedFile(String name, Uint8List data) =>
      _handle.embedFile(name, data);
  Future<void> eraseRegions(int pageIndex, List<PdfRect> regions) =>
      _handle.eraseRegions(pageIndex, regions);
  Future<void> flattenForms() => _handle.flattenForms();
  Future<void> flattenAllAnnotations() => _handle.flattenAllAnnotations();
  Future<void> applyAllRedactions() => _handle.applyAllRedactions();
  Future<void> setFormFieldValue(String fieldName, String value) =>
      _handle.setFormFieldValue(fieldName, value);
  Future<void> cropMargins({double left = 0, double right = 0,
      double top = 0, double bottom = 0}) =>
      _handle.cropMargins(left: left, right: right, top: top, bottom: bottom);
  Future<void> convertToPdfA({int level = 1}) =>
      _handle.convertToPdfA(level: level);
  Future<void> addWatermarkPositioned(int pageIndex, String text, {
    required double x, required double y,
    required double width, required double height,
    double fontSize = 48, String? fontName,
    double rotation = 45, double opacity = 0.3,
    double r = 0.5, double g = 0.5, double b = 0.5,
  }) => _handle.addWatermarkPositioned(pageIndex, text,
      x: x, y: y, width: width, height: height,
      fontSize: fontSize, fontName: fontName,
      rotation: rotation, opacity: opacity, r: r, g: g, b: b);

  Future<void> addStamp(int pageIndex, {
    required int stampType,
    String? customName,
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) => _handle.addStamp(pageIndex,
      stampType: stampType, customName: customName,
      x: x, y: y, width: width, height: height, opacity: opacity);

  Future<void> addImageStamp(int pageIndex, Uint8List imageBytes, {
    required double x, required double y,
    required double width, required double height,
    double opacity = 1.0,
  }) => _handle.addImageStamp(pageIndex, imageBytes,
      x: x, y: y, width: width, height: height, opacity: opacity);

  Future<void> resizeImage(int pageIndex, String imageName,
      {required double width, required double height}) =>
      _handle.resizeImage(pageIndex, imageName, width: width, height: height);

  Future<Uint8List> save() => _handle.save();
  Future<Uint8List> saveWithOptions({bool compress = true,
      bool garbageCollect = true, bool linearize = false}) =>
      _handle.saveWithOptions(compress: compress,
          garbageCollect: garbageCollect, linearize: linearize);
  Future<Uint8List> saveEncrypted({required String ownerPassword,
      String userPassword = ''}) =>
      _handle.saveEncrypted(ownerPassword: ownerPassword,
          userPassword: userPassword);
  Future<Uint8List> saveEncryptedFull({
      required String ownerPassword, String userPassword = '',
      int algorithm = 3,
      bool allowPrint = true, bool allowPrintHq = true,
      bool allowModify = true, bool allowCopy = true,
      bool allowAnnotate = true, bool allowFillForms = true,
      bool allowAccessibility = true, bool allowAssemble = true,
  }) => _handle.saveEncryptedFull(
      ownerPassword: ownerPassword, userPassword: userPassword,
      algorithm: algorithm,
      allowPrint: allowPrint, allowPrintHq: allowPrintHq,
      allowModify: allowModify, allowCopy: allowCopy,
      allowAnnotate: allowAnnotate, allowFillForms: allowFillForms,
      allowAccessibility: allowAccessibility, allowAssemble: allowAssemble);

  Future<void> dispose() => _handle.dispose();
}
