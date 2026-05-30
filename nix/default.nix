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
  pango,
  pkg-config,
  version ? "git",
  shortRev ? "unknown",
  gdkPixbufSupport ? true,
  pangoSupport ? true,
}:

rustPlatform.buildRustPackage {
  pname = "nlock";
  inherit version;

  src = ../.;

  cargoLock = {
    lockFile = ../Cargo.lock;
  };

  buildNoDefaultFeatures = true;
  buildFeatures =
    [ ] ++ (lib.optional gdkPixbufSupport "gdk-pixbuf") ++ (lib.optional pangoSupport "pango");

  nativeBuildInputs = [
    installShellFiles
    clang
    pkg-config
  ];

  buildInputs = [
    cairo
    glib
    libxkbcommon
    pam
  ]
  ++ (lib.optional gdkPixbufSupport gdk-pixbuf)
  ++ (lib.optional pangoSupport pango);

  postInstall = ''
    installShellCompletion --cmd nlock \
      --bash <($out/bin/nlock completions bash) \
      --zsh <($out/bin/nlock completions zsh) \
      --fish <($out/bin/nlock completions fish)
  '';

  LIBCLANG_PATH = "${clang.cc.lib}/lib";
  NLOCK_COMMIT = "${shortRev}"; # used to generate version string

  meta = with lib; {
    description = "Customisable, minimalist screen locker for Wayland";
    homepage = "https://github.com/OldUser101/nlock";
    license = licenses.gpl3Plus;
    platforms = platforms.linux;
    mainProgram = "nlock";
  };
}
