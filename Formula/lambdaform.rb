class Lambdaform < Formula
  desc "Terraform-native local Lambda emulator — no Docker, no CloudFormation"
  homepage "https://github.com/ConnerV42/lambdaform"
  version "0.4.0"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/ConnerV42/lambdaform/releases/download/v#{version}/lambdaform-macos-aarch64.tar.gz"
      sha256 "PLACEHOLDER_MACOS_AARCH64"
    end
    on_intel do
      url "https://github.com/ConnerV42/lambdaform/releases/download/v#{version}/lambdaform-macos-x86_64.tar.gz"
      sha256 "PLACEHOLDER_MACOS_X86_64"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/ConnerV42/lambdaform/releases/download/v#{version}/lambdaform-linux-x86_64.tar.gz"
      sha256 "PLACEHOLDER_LINUX_X86_64"
    end
  end

  def install
    # Release tarballs contain platform-specific binary names
    Dir["lambdaform-*"].each do |f|
      next if f.end_with?(".tar.gz")
      bin.install f => "lambdaform"
    end
    # Fallback if binary is just named "lambdaform"
    bin.install "lambdaform" if File.exist?("lambdaform") && !File.exist?(bin/"lambdaform")
  end

  test do
    assert_match "lambdaform", shell_output("#{bin}/lambdaform --version")
  end
end
