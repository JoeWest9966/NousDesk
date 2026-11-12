#!/usr/bin/env bash
#
# 出 Linux 包。产物全在 dist/：
#   nousdesk-<版本>-linux-<架构>.tar.gz   便携包（解压即用，二进制旁边带 desktop.toml 就生效）
#   nousdesk_<版本>_<架构>.deb            装到 /usr/bin，带 .desktop 和图标
#
# 用法：
#   scripts/package-linux.sh               # 编译 release + 出两个包
#   scripts/package-linux.sh --no-build    # 复用已有的 target/release/nousdesk
#
# 环境变量：
#   NOUS_HOST   有值就把它写进便携包的 desktop.toml（内网地址不该进公库，所以走环境变量）
#   NOUS_BOARD  同上，默认 new1
#
# 为什么不用 cargo-packager：它现在只把 Linux 当实验格式，msi/nsis 才是它的主场。
# tar.gz + dpkg-deb 是两段各二十行、能读懂的 shell，比多一个工具链好维护。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

BUILD=1
for arg in "$@"; do
  case "$arg" in
    --no-build) BUILD=0 ;;
    -h|--help) sed -n '2,17p' "$0"; exit 0 ;;
    *) echo "不认识的参数：$arg" >&2; exit 2 ;;
  esac
done

# 版本只有一个来源：workspace 的 [workspace.package] version。
VERSION="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)"
[ -n "$VERSION" ] || { echo "读不到 Cargo.toml 的 workspace version" >&2; exit 1; }

ARCH_DEB="$(dpkg --print-architecture 2>/dev/null || echo amd64)"
case "$ARCH_DEB" in
  amd64) ARCH_TAR=x64 ;;
  arm64) ARCH_TAR=aarch64 ;;
  *) ARCH_TAR="$(uname -m)" ;;
esac

BIN="$ROOT/target/release/nousdesk"
STAGE="$ROOT/dist/stage"
DIST="$ROOT/dist"

if [ "$BUILD" = 1 ]; then
  cargo build --release -p nousdesk
fi
[ -x "$BIN" ] || {
  echo "没有 $BIN —— 先跑 cargo build --release -p nousdesk（或去掉 --no-build）" >&2
  exit 1
}

rm -rf "$STAGE"
mkdir -p "$STAGE"

# ── 便携包：一个目录 = 一份安装，`cp -a` 走就是搬家 ─────────────────────────
PKG="$STAGE/nousdesk"
mkdir -p "$PKG"
install -m755 "$BIN" "$PKG/nousdesk"
install -m644 README.md LICENSE "$PKG/"
install -m644 assets/nousdesk.desktop "$PKG/nousdesk.desktop"
install -m644 assets/icon.png "$PKG/nousdesk.png"

if [ -n "${NOUS_HOST:-}" ]; then
  printf 'host = "%s"\nboard = "%s"\n' "$NOUS_HOST" "${NOUS_BOARD:-new1}" > "$PKG/desktop.toml"
else
  # 不塞死某个人的内网地址：给一份能照着改的样例，用户复制成 desktop.toml 就行。
  cat > "$PKG/desktop.toml.example" <<'TOML'
# 复制成 desktop.toml（和 nousdesk 同一个目录）就是这份安装的配置。
# 引擎地址不写在这儿 —— 它从宿主现拿，这里只写「去哪找宿主」。
host = "http://127.0.0.1:4790"
board = "new1"
TOML
fi

TARBALL="$DIST/nousdesk-$VERSION-linux-$ARCH_TAR.tar.gz"
tar -C "$STAGE" -czf "$TARBALL" nousdesk

# ── .deb ─────────────────────────────────────────────────────────────────
DEB="$STAGE/deb"
mkdir -p "$DEB/DEBIAN" \
         "$DEB/usr/bin" \
         "$DEB/usr/share/applications" \
         "$DEB/usr/share/icons/hicolor/256x256/apps" \
         "$DEB/usr/share/doc/nousdesk"

install -m755 "$BIN" "$DEB/usr/bin/nousdesk"
install -m644 assets/nousdesk.desktop "$DEB/usr/share/applications/nousdesk.desktop"
install -m644 assets/icon.png "$DEB/usr/share/icons/hicolor/256x256/apps/nousdesk.png"
install -m644 README.md "$DEB/usr/share/doc/nousdesk/README.md"
install -m644 LICENSE "$DEB/usr/share/doc/nousdesk/LICENSE"

# Installed-Size 的单位是 KiB 且按 1KiB 向上取整 —— dpkg-deb 自己不查，lintian 会。
INSTALLED_KIB="$(( ($(du -sk "$DEB/usr" | cut -f1)) ))"

# 运行期依赖全是 dlopen 或直接链接的：libvulkan / libEGL / libwayland 是 GPUI 运行时才 dlopen
# （所以它们只是 Recommends 级别也能起，但缺了会白屏，索性写进 Depends）。
cat > "$DEB/DEBIAN/control" <<CONTROL
Package: nousdesk
Version: $VERSION
Architecture: $ARCH_DEB
Maintainer: JoeWest9966 <JoeWest9966@users.noreply.github.com>
Installed-Size: $INSTALLED_KIB
Depends: libc6, libgcc-s1, libstdc++6, libxkbcommon0, libxkbcommon-x11-0, libxcb1, libxcb-xkb1, libvulkan1, libegl1, libgl1, libwayland-client0
Recommends: fonts-noto-cjk
Section: net
Priority: optional
Homepage: https://github.com/JoeWest9966/NousDesk
Description: NousDesk — Nous 桌面客户端
 Nous 的桌面客户端：三栏布局、会话、输入、置顶、New Manual 盘面。
 原生 GPUI 应用，不是网页；引擎地址从 Nous 宿主的 __nous/resolve 现拿。
CONTROL

DEBFILE="$DIST/nousdesk_${VERSION}_${ARCH_DEB}.deb"
dpkg-deb --root-owner-group --build "$DEB" "$DEBFILE" >/dev/null

echo
echo "出包完成（$VERSION / $ARCH_DEB）："
ls -lh "$TARBALL" "$DEBFILE" | sed 's/^/  /'
echo
echo "便携：  tar -xzf $(basename "$TARBALL") && ./nousdesk/nousdesk"
echo "安装：  sudo apt install ./$(basename "$DEBFILE")"
