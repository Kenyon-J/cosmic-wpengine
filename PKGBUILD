# Maintainer: Your Name <your.email@example.com>

pkgname=cosmic-wallpaper-git
pkgver=0.1.0.r0.g1234567
pkgrel=1
pkgdesc="A Wayland-native live wallpaper engine optimized for the COSMIC desktop"
arch=('x86_64')
url="https://github.com/Kenyon-J/cosmic-wpengine"
license=('MIT')
depends=('pipewire' 'ffmpeg' 'wayland' 'libxkbcommon' 'gcc-libs')
makedepends=('cargo' 'clang' 'git' 'pkgconf')
provides=('cosmic-wallpaper')
conflicts=('cosmic-wallpaper')
source=("git+https://github.com/Kenyon-J/cosmic-wpengine.git")
sha256sums=('SKIP')

pkgver() {
  cd cosmic-wpengine
  # Generate git version (e.g. 0.1.0.r14.g8a2c4)
  git describe --long --tags --always | sed 's/\([^-]*-g\)/r\1/;s/-/./g'
}

build() {
  cd cosmic-wpengine
  # Build the engine and the GUI
  cargo build --release --locked --all-targets
}

package() {
  cd cosmic-wpengine
  
  # Install binaries
  install -Dm755 target/release/cosmic-wallpaper "$pkgdir/usr/bin/cosmic-wallpaper"
  install -Dm755 target/release/cosmic-wallpaper-gui "$pkgdir/usr/bin/cosmic-wallpaper-gui"
}