{
  description = "Gup Rust development environment";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
          config.allowUnfree = true;
        };

        # rust-toolchain.toml pins the version, components and targets; CI
        # installs the same file, so local and CI toolchains cannot drift.
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        chromium-webgpu = pkgs.writeShellScriptBin "chromium-webgpu" ''
          exec chromium --enable-features=WebGPU,Vulkan --enable-unsafe-webgpu --disable-dawn-features=disallow_unsafe_apis "$@"
        '';

        # Nix stdenv bash is compiled without readline, which breaks
        # tools that rely on interactive bash sessions (e.g. GitHub
        # Copilot CLI). Include bashInteractive so it appears first
        # in PATH.

        # rely on PATH (or devShell) to avoid store change issues.
        # `mask pre-commit` scopes the checks to the staged files (GUP-398);
        # `mask all-check` is the full gate, and CI runs it on every push.
        pre-commit = pkgs.writeScript "gup-pre-commit" ''
          mask pre-commit
        '';
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = with pkgs; [
            bashInteractive
            rustToolchain
            pkg-config

            # Graphics and windowing libraries for wgpu
            libxkbcommon
            libGL

            # Wayland support
            wayland

            # X11 support
            libx11
            libxcursor
            libxi
            libxrandr

            # Vulkan support
            vulkan-loader
            vulkan-headers
            vulkan-validation-layers

            # Mesa drivers for OpenGL/Vulkan
            mesa

            # OpenSSL for reqwest and networking
            openssl
            openssl.dev

            # Additional development tools
            cargo-watch
            cargo-edit
            cargo-audit
            mask
            git
            gnused
            wasm-pack
            miniserve
            mprocs
            concurrently
            mdl
            nixfmt
            statix

            # pyftsubset for `mask subset-inter` (GUP-407): the bundled
            # Inter subset is reproducible with this pinned fonttools.
            python3Packages.fonttools

            # Workflow lint (GUP-409). nixpkgs wraps actionlint with
            # shellcheck, which it runs on every `run:` script.
            actionlint

            # Headless X server: `mask ci dogfood` runs the windowed tasks
            # under Xvfb, as the Dogfood workflow does.
            xvfb-run

            # Node.js for Puppeteer benchmark capture scripts
            nodejs

            # Prettier for markdown formatting
            nodePackages.prettier

            # WebGPU-enabled Chromium and matching ChromeDriver for headless tests
            chromium-webgpu
            chromedriver
          ];

          shellHook = ''
            export RUST_BACKTRACE=1

            # Build output (GUP-411): one build directory shared by every
            # checkout, and a target directory of this checkout's own (see
            # "Sharing build output between checkouts" in CLAUDE.md). Not in
            # CI, which caches ./target, nor over an explicit CARGO_TARGET_DIR.
            top="$(git rev-parse --show-toplevel 2>/dev/null || true)"
            if [[ -z ''${CI:-} && -z ''${CARGO_TARGET_DIR:-} && -x $top/scripts/cargo_env.sh ]]; then
              eval "$("$top/scripts/cargo_env.sh")"
            fi

            # Set up Mesa drivers. GUP_SOFTWARE_GPU=1 selects lavapipe
            # (software Vulkan) instead: CI runners have no GPU, and the
            # same switch reproduces CI locally.
            export LIBGL_DRIVERS_PATH="${pkgs.mesa}/lib/dri"
            export GUP_LAVAPIPE_ICD="${pkgs.mesa}/share/vulkan/icd.d/lvp_icd.x86_64.json"
            if [[ -n "''${GUP_SOFTWARE_GPU:-}" ]]; then
              export VK_ICD_FILENAMES="$GUP_LAVAPIPE_ICD"
            else
              export VK_ICD_FILENAMES="${pkgs.mesa}/share/vulkan/icd.d/radeon_icd.x86_64.json:${pkgs.mesa}/share/vulkan/icd.d/intel_icd.x86_64.json"
            fi

            # Install the pre-commit hook, and replace a symlink to an older
            # version of it (a regular-file hook is the user's own: keep it).
            hook="$(git rev-parse --git-path hooks/pre-commit 2>/dev/null || true)"
            if [[ -n $hook && ( ! -e $hook || -L $hook ) && "$(readlink "$hook")" != "${pre-commit}" ]]; then
              echo "Setting up git pre-commit hook..."
              mkdir -p "$(dirname "$hook")"
              ln -sfn ${pre-commit} "$hook"
            fi

            echo "🦀 Rust development environment loaded!"
            echo "Rust version: $(rustc --version)"
            echo "Cargo version: $(cargo --version)"
            echo "Cargo build dir: ''${CARGO_BUILD_BUILD_DIR:-default}, target dir: ''${CARGO_TARGET_DIR:-default}"
            echo "Chromium available for WebGPU testing: $(chromium --version 2>/dev/null || echo 'Not found')"
            echo "ChromeDriver version: $(chromedriver --version 2>/dev/null || echo 'Not found')"
            echo "Node.js version: $(node --version 2>/dev/null || echo 'Not found')"
            echo "🌐 Use 'chromium-webgpu http://127.0.0.1:8080' to test WebGPU apps"
          '';

          # Set environment variables for graphics libraries
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath [
            pkgs.libGL
            pkgs.libxkbcommon
            pkgs.wayland
            pkgs.libx11
            pkgs.libxcursor
            pkgs.libxi
            pkgs.libxrandr
            pkgs.vulkan-loader
            pkgs.openssl
          ];
        };
      }
    );
}
