{ pkgs }: {
  deps = [
    pkgs.rustc
    pkgs.cargo
    pkgs.rust-analyzer
    pkgs.pkg-config
    pkgs.openssl
  ];
}
