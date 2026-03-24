# Maintainer: Your Name <josh@jkenyon.co.uk>

pkgname=cosmic-wallpaper-git
pkgver=0.1.0.r0.g1
pkgrel=1
pkgdesc="A live wallpaper engine for the COSMIC desktop"
arch=('x86_64')
url="https://github.com/Kenyon-J/cosmic-wpengine"
license=('MIT')
# Runtime dependencies based on your Rust crates and command-line invocations
depends=('gcc-libs' 'glibc' 'wayland' 'pipewire' 'libxkbcommon' 'dbus' 'ffmpeg' 'noto-fonts' 'ttf-dejavu')
makedepends=('cargo' 'pkgconf' 'git')
provides=('cosmic-wallpaper' 'cosmic-wallpaper-gui')
conflicts=('cosmic-wallpaper' 'cosmic-wallpaper-gui')
source=("git+https://github.com/Kenyon-J/cosmic-wpengine.git")
md5sums=('SKIP')

pkgver() {
  cd cosmic-wallpaper
  # Generate a version string based on the latest git commit and tag
  git describe --long --tags --abbrev=7 2>/dev/null | sed 's/\([^-]*-g\)/r\1/;s/-/./g' ||
  printf "r%s.%s" "$(git rev-list --count HEAD)" "$(git rev-parse --short=7 HEAD)"
}

prepare() {
  cd cosmic-wallpaper
  export RUSTUP_TOOLCHAIN=stable
  cargo fetch --locked --target "$(rustc -vV | sed -n 's/host: //p')"
}

build() {
  cd cosmic-wallpaper
  export RUSTUP_TOOLCHAIN=stable
  cargo build --frozen --release --all-targets
}

package() {
  cd cosmic-wallpaper
  install -Dm0755 target/release/cosmic-wallpaper "$pkgdir/usr/bin/cosmic-wallpaper"
  install -Dm0755 target/release/cosmic-wallpaper-gui "$pkgdir/usr/bin/cosmic-wallpaper-gui"
}