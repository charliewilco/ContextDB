# typed: false
# frozen_string_literal: true

# Homebrew formula for ContextDB
#
# To install locally (for testing):
#   brew install --build-from-source ./Formula/contextdb.rb
#
# To tap and install:
#   brew tap yourusername/contextdb https://github.com/yourusername/contextdb
#   brew install contextdb
#

class Contextdb < Formula
  desc "A semantic database for LLM applications with human-readable inspection"
  homepage "https://github.com/yourusername/contextdb"
  url "https://github.com/yourusername/contextdb/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "REPLACE_WITH_ACTUAL_SHA256_AFTER_RELEASE"
  license "MIT"
  head "https://github.com/yourusername/contextdb.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args
  end

  test do
    # Create a test database
    system "#{bin}/contextdb", "init", "test.db"
    assert_predicate testpath/"test.db", :exist?

    # Check stats
    output = shell_output("#{bin}/contextdb stats test.db")
    assert_match "Entries: 0", output
  end
end
