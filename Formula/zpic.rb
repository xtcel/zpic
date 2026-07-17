# This file is a template source-of-truth for the Homebrew tap formula.
# The release workflow renders a concrete copy with versioned URLs and
# per-platform checksums, then uploads it as a release asset named
# `zpic.rb`. It installs the prebuilt binary for the running platform
# instead of compiling from source, so it does not pull in a Rust/LLVM
# toolchain on the end user's machine.

class Zpic < Formula
  desc "Rust-native image hosting CLI compatible with PicGo configuration"
  homepage "https://github.com/@@REPO@@"
  version "@@VERSION@@"
  license any_of: ["MIT", "Apache-2.0"]

  on_macos do
    on_arm do
      url "https://github.com/@@REPO@@/releases/download/v@@VERSION@@/zpic-v@@VERSION@@-aarch64-apple-darwin.tar.gz"
      sha256 "@@MACOS_ARM64_SHA256@@"
    end
    on_intel do
      url "https://github.com/@@REPO@@/releases/download/v@@VERSION@@/zpic-v@@VERSION@@-x86_64-apple-darwin.tar.gz"
      sha256 "@@MACOS_X86_64_SHA256@@"
    end
  end

  on_linux do
    url "https://github.com/@@REPO@@/releases/download/v@@VERSION@@/zpic-v@@VERSION@@-x86_64-unknown-linux-gnu.tar.gz"
    sha256 "@@LINUX_X86_64_SHA256@@"
  end

  def install
    bin.install "zpic"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/zpic --version")
  end
end
