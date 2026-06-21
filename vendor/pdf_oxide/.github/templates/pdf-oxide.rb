class PdfOxide < Formula
  desc "The fastest PDF toolkit — extract text, images, metadata, and more"
  homepage "https://github.com/yfedoseev/pdf_oxide"
  version "{{VERSION}}"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yfedoseev/pdf_oxide/releases/download/v{{VERSION}}/pdf_oxide-macos-aarch64-{{VERSION}}.tar.gz"
      sha256 "{{SHA256_MACOS_ARM}}"
    else
      url "https://github.com/yfedoseev/pdf_oxide/releases/download/v{{VERSION}}/pdf_oxide-macos-x86_64-{{VERSION}}.tar.gz"
      sha256 "{{SHA256_MACOS_X86}}"
    end
  end

  on_linux do
    url "https://github.com/yfedoseev/pdf_oxide/releases/download/v{{VERSION}}/pdf_oxide-linux-x86_64-musl-{{VERSION}}.tar.gz"
    sha256 "{{SHA256_LINUX_X86}}"
  end

  def install
    bin.install "pdf-oxide"
    bin.install "pdf-oxide-mcp"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/pdf-oxide --version")
  end
end
