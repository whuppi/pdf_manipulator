// Link hook — passthrough. Forwards every native asset to the app
// bundle unchanged.
//
// Do NOT delete this file as "dead code": on release/AOT builds
// Flutter routes native assets through the link hook (build.dart
// emits them ToLinkHook), and a missing hook fails the build. Debug
// builds bypass it (assets route ToAppBundle); web assets never
// pass through here.
//
// Feature-trimming plans (cargo features via user_defines, @RecordUse
// tree-shaking) live in the capability roadmap, not here.

import 'package:hooks/hooks.dart';

void main(List<String> args) async {
  await link(args, (LinkInput input, LinkOutputBuilder output) async {
    for (final asset in input.assets.encodedAssets) {
      output.assets.addEncodedAsset(asset);
    }
  });
}
