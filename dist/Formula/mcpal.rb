# Homebrew formula template for mcpal. cargo-dist regenerates this when a
# release is tagged; until then `brew install --HEAD pawelb0/tap/mcpal`
# builds from `main`.
class Mcpal < Formula
  desc "Scriptable command-line client for the Model Context Protocol"
  homepage "https://github.com/pawelb0/mcpal"
  license "MIT"
  head "https://github.com/pawelb0/mcpal.git", branch: "main"

  # Stub formula. cargo-dist (or a manual tap commit) replaces this with a
  # versioned bottle stanza once `v0.x.0` ships. Until then `brew install
  # --HEAD pawelb0/tap/mcpal` builds from source on `main`.

  depends_on "rust" => :build

  def install
    cd "crates/mcpal-cli" do
      system "cargo", "install", *std_cargo_args(path: ".")
    end
  end

  test do
    assert_match "mcpal", shell_output("#{bin}/mcpal --version")
  end
end
