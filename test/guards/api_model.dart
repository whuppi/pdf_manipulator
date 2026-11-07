// The public API as the analyzer resolves it: every name the two barrels
// export, with its members, their parameters and their types. The naming and
// signature walls read this one model, so a rule is checked against what an
// app can call and never against what a regex happens to see.
//
// io-exempt: the analyzer reads lib from disk.

import 'dart:io';

import 'package:analyzer/dart/analysis/analysis_context_collection.dart';
import 'package:analyzer/dart/analysis/results.dart';
import 'package:analyzer/dart/element/element.dart';

/// A public member of an exported type: a method, a constructor, a getter or
/// a field.
class ApiMember {
  ApiMember(this.owner, this.element);

  /// The exported type that declares it.
  final InterfaceElement owner;

  /// The analyzer's element for it.
  final Element element;

  /// The member's name; a constructor's is `Type` or `Type.name`.
  String get name => element.displayName;

  /// Its parameters, empty for a field or getter.
  List<FormalParameterElement> get parameters => switch (element) {
    ExecutableElement(:final formalParameters) => formalParameters,
    _ => const [],
  };

  /// `Owner.member`, for messages.
  String get label => '${owner.displayName}.$name';

  /// Whether it is a method that is neither a getter, a setter nor an
  /// operator.
  bool get isMethod =>
      element is MethodElement && !(element as MethodElement).isOperator;

  /// Whether it is a getter or a field.
  bool get isProperty => element is GetterElement || element is FieldElement;

  /// Whether it is a constructor.
  bool get isConstructor => element is ConstructorElement;

  /// The return type as source, or the field type.
  String get typeName => switch (element) {
    ExecutableElement(:final returnType) => returnType.getDisplayString(),
    FieldElement(:final type) => type.getDisplayString(),
    _ => '',
  };
}

/// The resolved public API of the package.
class ApiModel {
  ApiModel._(this.main, this.io);

  /// The elements `pdf_manipulator.dart` exports, by name.
  final Map<String, Element> main;

  /// The elements `io.dart` exports, by name.
  final Map<String, Element> io;

  /// Every exported element of both barrels, by name.
  Map<String, Element> get all => {...main, ...io};

  /// Every exported type (class, enum, extension type, mixin).
  Iterable<InterfaceElement> get types =>
      all.values.whereType<InterfaceElement>();

  /// Every public member of [type] it declares itself.
  Iterable<ApiMember> membersOf(InterfaceElement type) sync* {
    for (final c in type.constructors) {
      if (!c.isPrivate) yield ApiMember(type, c);
    }
    for (final m in type.methods) {
      if (!m.isPrivate) yield ApiMember(type, m);
    }
    for (final g in type.getters) {
      if (!g.isPrivate && g.isOriginDeclaration) yield ApiMember(type, g);
    }
    for (final f in type.fields) {
      if (!f.isPrivate && f.isOriginDeclaration && !f.isEnumConstant) {
        yield ApiMember(type, f);
      }
    }
  }

  /// Every public member of every exported type.
  Iterable<ApiMember> get members sync* {
    for (final type in types) {
      yield* membersOf(type);
    }
  }

  /// The type exported as [name], or fails.
  InterfaceElement type(String name) {
    final element = all[name];
    if (element is! InterfaceElement) {
      throw StateError('$name is not an exported type');
    }
    return element;
  }

  /// Loads the model from lib on disk.
  static Future<ApiModel> load() async {
    final root = Directory.current.absolute.path;
    final analysis = AnalysisContextCollection(includedPaths: ['$root/lib']);
    Future<Map<String, Element>> exports(String barrel) async {
      final session = analysis.contextFor('$root/lib/$barrel').currentSession;
      final result = await session.getLibraryByUri(
        'package:pdf_manipulator/$barrel',
      );
      if (result is! LibraryElementResult) {
        throw StateError('the analyzer could not resolve $barrel: $result');
      }
      return {
        for (final e in result.element.exportNamespace.definedNames2.entries)
          if (!e.key.startsWith('_')) e.key: e.value,
      };
    }

    return ApiModel._(
      await exports('pdf_manipulator.dart'),
      await exports('io.dart'),
    );
  }
}

/// One parameter of a public signature, as the walls see it.
class Par {
  /// Creates a parameter description.
  const Par(
    this.name,
    this.type, {
    this.isNamed = false,
    this.isRequired = false,
    this.defaultValue,
  });

  /// The parameter's name.
  final String name;

  /// Its type as source, `?` included.
  final String type;

  /// Whether it is a named parameter.
  final bool isNamed;

  /// Whether it is `required`.
  final bool isRequired;

  /// The source of its default value, if it has one.
  final String? defaultValue;
}

/// A public method, constructor, getter or field, flattened for the walls.
class Sig {
  /// Creates a signature description.
  const Sig(
    this.owner,
    this.name,
    this.kind, {
    this.params = const [],
    this.type = '',
  });

  /// The declaring type.
  final String owner;

  /// The member name; a constructor's is its own name or `named`.
  final String name;

  /// `method`, `constructor`, `getter` or `field`.
  final String kind;

  /// The parameters, in declaration order.
  final List<Par> params;

  /// The return type of a method or getter, the type of a field.
  final String type;

  /// `Owner.name`, for messages.
  String get label => '$owner.$name';
}

/// The signatures of every public member of every exported type.
List<Sig> signaturesOf(ApiModel model) => [
  for (final m in model.members)
    Sig(
      m.owner.displayName,
      m.isConstructor ? m.name.split('.').last : m.name,
      m.isConstructor
          ? 'constructor'
          : m.isMethod
          ? 'method'
          : m.element is MethodElement
          ? 'operator'
          : m.element is GetterElement
          ? 'getter'
          : 'field',
      type: m.typeName,
      params: [
        for (final p in m.parameters)
          Par(
            p.name ?? '',
            p.type.getDisplayString(),
            isNamed: p.isNamed,
            isRequired: p.isRequiredNamed,
            defaultValue: p.defaultValueCode,
          ),
      ],
    ),
];
