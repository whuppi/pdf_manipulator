// What each `build:` mode asks of cargo and of wasm-opt. The Asyncify wasm
// takes the same wasm-opt flags as the wasm it is made from, so a size build
// is size-optimized in both and no pass runs at a level the mode did not pick.

import 'package:pdf_manipulator/src/hook/engine_build.dart';
import 'package:test/test.dart';

void main() {
  test('speed is cargo release as is, wasm-opt at -O2', () {
    expect(EngineBuild.speed.cargoEnv, isEmpty);
    expect(EngineBuild.speed.binaryenArgs, ['--opt=2']);
  });

  test('size lowers both cargo and wasm-opt to z', () {
    expect(EngineBuild.size.cargoEnv, {'CARGO_PROFILE_RELEASE_OPT_LEVEL': 'z'});
    expect(EngineBuild.size.binaryenArgs, ['--opt=z']);
  });

  test('debug keeps symbols in cargo and names in wasm-opt', () {
    expect(EngineBuild.debug.cargoEnv, {
      'CARGO_PROFILE_RELEASE_DEBUG': '2',
      'CARGO_PROFILE_RELEASE_STRIP': 'none',
    });
    expect(EngineBuild.debug.binaryenArgs, ['--opt=2', '--keep-names']);
  });
}
