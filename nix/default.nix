{
  lib,
  rustPlatform,
  installShellFiles,
  cairo,
  clang,
  gdk-pixbuf,
  glib,
  libxkbcommon,
  pam,
  pkg-config,
  version ? "git",
}:

rustPlatform.buildRustPackage {
  pname = "nlock";
  inherit version;

  src = ../.;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  nativeBuildInputs = [
    installShellFiles
    clang
    pkg-config
  ];

  buildInputs = [
    cairo
    gdk-pixbuf
    glib
    libxkbcommon
    pam
  ];

  postInstall = ''
    installShellCompletion --cmd nlock \
      --bash <($out/bin/nlock completions bash) \
      --zsh <($out/bin/nlock completions zsh) \
      --fish <($out/bin/nlock completions fish)
  '';

  LIBCLANG_PATH = "${clang.cc.lib}/lib";

  meta = with lib; {
    description = "Customisable, minimalist screen locker for Wayland";
    homepage = "https://github.com/OldUser101/nlock";
    license = licenses.gpl3Plus;
    platforms = platforms.linux;
    mainProgram = "nlock";
  };
}
