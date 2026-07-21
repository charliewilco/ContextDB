# typed: false
# frozen_string_literal: true

# Homebrew formula for ContextDB
#
# To install locally (for testing):
#   brew install --HEAD ./Formula/contextdb.rb
#
# To tap and install:
#   brew tap charliewilco/contextdb https://github.com/charliewilco/contextdb
#   brew install --HEAD contextdb
#

class Contextdb < Formula
  desc "A semantic database for LLM applications with human-readable inspection"
  homepage "https://github.com/charliewilco/contextdb"
  license "MIT"
  head "https://github.com/charliewilco/contextdb.git", branch: "main"

  depends_on "rust" => :build

  def install
    system "cargo", "install", *std_cargo_args, "--features", "cli"
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
