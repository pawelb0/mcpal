# Homebrew formula template for mcpal. cargo-dist will regenerate this
# file (and publish it to pawelb/homebrew-tap) on each release; the
# placeholders below let `brew install --HEAD` work in the meantime.
class Mcpal < Formula
  desc "Scriptable command-line client for the Model Context Protocol"
  homepage "https://github.com/pawelb/mcpal"
  license any_of: ["MIT", "Apache-2.0"]
  head "https://github.com/pawelb/mcpal.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args(path: "crates/mcpal-cli")
  end

  test do
    assert_match "mcpal", shell_output("#{bin}/mcpal --version")
  end
end
