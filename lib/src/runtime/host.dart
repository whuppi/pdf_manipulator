// Platform-selected LaneHost factory. The default is the neutral stub (no
// platform library) so pub.dev's analyzer keeps web supported; dart.library.io
// resolves the native lane runtime, dart.library.js_interop the web one. The
// hosts live inside their lane libraries (part of), so those libraries are the
// conditional-import targets, each exposing createLaneHost.

import 'package:pdf_manipulator/src/runtime/lane.dart';
import 'package:pdf_manipulator/src/types/pdf_config.dart';

import 'package:pdf_manipulator/src/runtime/host_stub.dart'
    if (dart.library.io) 'package:pdf_manipulator/src/runtime/native/lane.dart'
    if (dart.library.js_interop) 'package:pdf_manipulator/src/runtime/web/lane.dart'
    as impl;

/// Creates the platform's [LaneHost] via conditional import.
LaneHost createLaneHost({PdfConfig? config}) =>
    impl.createLaneHost(config: config);
