/// UI-test harness + the example app's robots.
///
/// One package: import gives the host-VM journeys (test/) and the
/// on-device smoke suite (integration_test/) everything they need —
/// device matrix, base Robot primitives, pump strategies, and the
/// app-specific robots — with no relative path reaching across test
/// roots (the ../ form that breaks web integration tests).
library;

// Harness — app-agnostic, reusable in any Flutter app.
export 'src/harness/device_matrix.dart';
export 'src/harness/device_profiles.dart';
export 'src/harness/pump_strategies.dart';
export 'src/harness/robot.dart';

// Robots — specific to the example app under test.
export 'src/robots/app_robot.dart';
export 'src/robots/runtime_tab_robot.dart';
