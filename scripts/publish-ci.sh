#!/usr/bin/env bash
#
# 把**本地/远程机上出好的**包按提交号挂到滚动预发布 `ci`。
#
# 为什么要有这个脚本：推 main 时 `.github/workflows/linux.yml` 会自动出一份挂上去，
# 但 CI 是 4 核、含测试和打包，一轮 11–15 分钟。想马上在一台机器上试某个改动时，
# 在开发机上 `scripts/package-linux.sh` 出包（增量几十秒）再跑这个脚本，2 分钟就到手。
# 两条路**落同一个名字**（`nousdesk-<sha8>-linux-x64.tar.gz`），后到的覆盖。
#
# 用法：
#   ./scripts/publish-ci.sh                    # 用 dist/ 里现成的包
#   ./scripts/publish-ci.sh dist/              # 指定目录
#   ./scripts/publish-ci.sh --dry-run          # 只打印要做什么，不碰网络
#   ./scripts/publish-ci.sh --dirty            # 工作树不干净也发（名字带 -dirty）
#
# 环境：
#   GH_TOKEN         有 repo 写权限的 token；不设就用 `gh auth` 里那个
#   NOUS_CI_RELEASE  滚动发布的名字，默认 ci
#   NOUS_CI_KEEP     保留几份，默认 5
#
# 退出码：2 = 用法错；1 = 前置检查没过（dist 里没有包 / 工作树脏）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

RELEASE="${NOUS_CI_RELEASE:-ci}"
KEEP="${NOUS_CI_KEEP:-5}"

DRY=0
DIRTY_OK=0
DIST="$ROOT/dist"
for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY=1 ;;
    --dirty) DIRTY_OK=1 ;;
    -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
    -*) echo "不认识的参数：$arg" >&2; exit 2 ;;
    *) DIST="$arg" ;;
  esac
done

die() { echo "✗ $*" >&2; exit 1; }
say() { echo "· $*"; }

TAR_SRC="$(ls -1 "$DIST"/*.tar.gz 2>/dev/null | head -1 || true)"
DEB_SRC="$(ls -1 "$DIST"/*.deb 2>/dev/null | head -1 || true)"
[ -n "$TAR_SRC" ] || die "$DIST 里没有 *.tar.gz
  先出包：env -u CARGO_TARGET_DIR scripts/package-linux.sh"
[ -n "$DEB_SRC" ] || die "$DIST 里没有 *.deb
  先出包：env -u CARGO_TARGET_DIR scripts/package-linux.sh"

git rev-parse --git-dir >/dev/null 2>&1 || die "不在 git 仓里，取不到提交号"
SHA="$(git rev-parse --short=8 HEAD)"
SUFFIX=""
if [ -n "$(git status --porcelain)" ]; then
  [ "$DIRTY_OK" = 1 ] || die "工作树不干净：包对应不了任何一个提交
  先提交，或者加 --dirty（名字里会带 -dirty）"
  SUFFIX="-dirty"
  say "警告：工作树不干净，名字带 -dirty"
fi

# deb 里的 Version 仍是 Cargo.toml 的版本（文件名带 sha 只为对应提交）。
VER="$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)"

STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
OUT_TAR="nousdesk-$SHA$SUFFIX-linux-x64.tar.gz"
OUT_DEB="nousdesk_${SHA}${SUFFIX}_amd64.deb"
cp "$TAR_SRC" "$STAGE/$OUT_TAR"
cp "$DEB_SRC" "$STAGE/$OUT_DEB"
( cd "$STAGE" && sha256sum "$OUT_TAR" "$OUT_DEB" > "nousdesk-$SHA$SUFFIX.sha256" )

say "提交  $SHA$SUFFIX"
say "版本  $VER（deb 的 Version 字段，不涨号）"
say "包    $OUT_TAR  $(du -h "$STAGE/$OUT_TAR" | cut -f1) / $OUT_DEB  $(du -h "$STAGE/$OUT_DEB" | cut -f1)"

if [ "$DRY" = 1 ]; then
  say "--dry-run：要执行的是"
  echo "    gh release view $RELEASE || gh release create $RELEASE --prerelease --target main …"
  echo "    gh release upload $RELEASE $OUT_TAR $OUT_DEB nousdesk-$SHA$SUFFIX.sha256 --clobber"
  echo "    # 只留最近 $KEEP 份（按上传时间）"
  exit 0
fi

REPO_ARG=()
if [ -n "${GH_REPO:-}" ]; then REPO_ARG=(--repo "$GH_REPO"); fi

if ! gh release view "$RELEASE" "${REPO_ARG[@]}" >/dev/null 2>&1; then
  say "建滚动预发布 $RELEASE"
  gh release create "$RELEASE" "${REPO_ARG[@]}" \
    --prerelease \
    --target main \
    --title "CI 构建" \
    --notes "按提交号命名的包（\`git rev-parse --short=8\`）。推 main 时 CI 自动出一份，开发机上跑 \`scripts/publish-ci.sh\` 也能补一份同名覆盖。只留最近 $KEEP 份；正式版本在 \`v*\` 的 Release 里。"
fi

say "上传到 $RELEASE"
gh release upload "$RELEASE" "${REPO_ARG[@]}" \
  "$STAGE/$OUT_TAR" "$STAGE/$OUT_DEB" "$STAGE/nousdesk-$SHA$SUFFIX.sha256" --clobber

say "清理：只留最近 $KEEP 份"
gh release view "$RELEASE" "${REPO_ARG[@]}" --json assets \
  -q '.assets[] | "\(.createdAt)\t\(.name)"' \
  | sort -r | tail -n +$((KEEP * 3 + 1)) | cut -f2 \
  | while read -r old; do
      [ -n "$old" ] && gh release delete-asset "$RELEASE" "$old" "${REPO_ARG[@]}" -y >/dev/null
    done

say "完成：$(gh release view "$RELEASE" "${REPO_ARG[@]}" --json url -q .url)"
echo
echo "取货："
echo "  gh release download $RELEASE -p 'nousdesk-$SHA$SUFFIX*'"
echo "装："
echo "  sudo apt install ./$OUT_DEB      # 或 tar -xzf $OUT_TAR 后直接跑"
