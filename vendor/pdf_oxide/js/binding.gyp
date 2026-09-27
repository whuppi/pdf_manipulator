{
  "targets": [
    {
      "target_name": "pdf_oxide",
      "sources": ["binding.cc"],
      "include_dirs": [
        "<!@(node -p \"require('node-addon-api').include\")"
      ],
      "dependencies": [
        "<!(node -p \"require('node-addon-api').gyp\")"
      ],
      "cflags": ["-Wall", "-Wextra"],
      "cflags_cc": ["-fexceptions", "-std=c++20"],
      "xcode_settings": {
        "GCC_ENABLE_CPP_EXCEPTIONS": "YES",
        "CLANG_CXX_LIBRARY": "libc++",
        "CLANG_CXX_LANGUAGE_STANDARD": "c++20",
        "MACOSX_DEPLOYMENT_TARGET": "11.0",
        "OTHER_LDFLAGS": [
          "-framework", "CoreFoundation",
          "-framework", "Security",
          "-framework", "SystemConfiguration"
        ]
      },
      "msvs_settings": {
        "VCCLCompilerTool": {
          "ExceptionHandling": 1,
          "AdditionalOptions": ["/std:c++20"]
        }
      },
      "conditions": [
        ["OS==\"linux\"", {
          "libraries": [
            "<(module_root_dir)/../lib/libpdf_oxide.a",
            "-lm",
            "-lpthread",
            "-ldl",
            "-lrt",
            "-lutil"
          ],
          "ldflags": [
            "-static-libstdc++",
            "-static-libgcc"
          ]
        }],
        ["OS==\"mac\"", {
          "libraries": [
            "<(module_root_dir)/../lib/libpdf_oxide.a",
            "-liconv",
            "-lresolv"
          ]
        }],
        ["OS==\"win\"", {
          "libraries": [
            "<(module_root_dir)/../lib/pdf_oxide.lib",
            "ws2_32.lib",
            "userenv.lib",
            "bcrypt.lib",
            "advapi32.lib",
            "crypt32.lib",
            "ntdll.lib",
            "kernel32.lib",
            "ole32.lib",
            "shell32.lib",
            "synchronization.lib"
          ]
        }]
      ]
    }
  ]
}
