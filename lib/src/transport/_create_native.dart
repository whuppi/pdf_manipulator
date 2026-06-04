import 'package:pdf_manipulator/src/transport/pdf_bridge.dart';
import 'package:pdf_manipulator/src/transport/shared_bridge.dart';
import 'package:pdf_manipulator/src/transport/native/native_transport.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

/// Creates a [PdfBridge] backed by native FFI transport.
PdfBridge createBridge({PdfConfig? config}) =>
    SharedBridge(NativeTransport());
