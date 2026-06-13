// Device profiles — the curated real-device matrix the UI harness runs
// every journey against.
//
// App-agnostic: copy this file (and the rest of harness/) into any
// Flutter app. The set brackets the real device-shape space — from a
// phone smaller than any CI emulator to a desktop window — so a journey
// that passes every profile here is proven for every device a real user
// or a CI runner might use.
//
// The smallest profile (320×568, the original iPhone SE) is deliberately
// narrower AND shorter than the Android CI emulator (≈360×640). That's
// the load-bearing guarantee: local exercises a tighter viewport than CI
// ever will, so local-green implies CI-green. Add a device = one row.

import 'dart:ui' show Size;

/// One device shape: a logical-pixel viewport plus its pixel density.
class DeviceProfile {
  const DeviceProfile({
    required this.name,
    required this.size,
    required this.devicePixelRatio,
  });

  /// Short label, used in test names (e.g. 'iPhoneSE').
  final String name;

  /// Logical size in dp — what layout sees.
  final Size size;

  /// Logical→physical scale. Real on-device value; affects text/raster
  /// rounding that can shift hit-test points by a sub-pixel.
  final double devicePixelRatio;

  @override
  String toString() =>
      '$name (${size.width.toInt()}×${size.height.toInt()} @${devicePixelRatio}x)';
}

/// The curated matrix. Ordered smallest-viewport first so the tightest
/// layout is exercised earliest.
const kDeviceMatrix = <DeviceProfile>[
  DeviceProfile(
    name: 'iPhoneSE',
    size: Size(320, 568),
    devicePixelRatio: 2.0,
  ),
  DeviceProfile(
    name: 'pixelSmall',
    size: Size(360, 640),
    devicePixelRatio: 2.0,
  ),
  DeviceProfile(
    name: 'iPhone15Pro',
    size: Size(393, 852),
    devicePixelRatio: 3.0,
  ),
  DeviceProfile(
    name: 'foldOpen',
    size: Size(768, 1080),
    devicePixelRatio: 2.0,
  ),
  DeviceProfile(
    name: 'iPadPro',
    size: Size(1024, 1366),
    devicePixelRatio: 2.0,
  ),
  DeviceProfile(
    name: 'desktop',
    size: Size(1440, 900),
    devicePixelRatio: 1.0,
  ),
];
