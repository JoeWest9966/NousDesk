#!/usr/bin/env bash
#
# 验剪贴板**读进来**的两段：平台读（系统剪贴板 → 我们）+ 粘贴（组件把读到的插进文本）。
#
# 写出去那一段验不了：Wayland 的 `set_selection` 必须带最近一次输入事件的 serial，
# 脚本里没有真实输入，合成器会丢掉请求 —— 看起来像「写剪贴板坏了」，其实是测试方式不对。
# 写的那一段是人工的：`cargo run --example clipboard` 里 Ctrl+A / Ctrl+C，再 `wl-paste` 对。
#
# 用法：scripts/check-clipboard.sh          # 需要 WAYLAND_DISPLAY + wl-clipboard
# 退出码：0 通过；1 失败；77 跳过（没 Wayland 或没装 wl-clipboard）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [ -z "${WAYLAND_DISPLAY:-}" ]; then
  echo "跳过：没有 WAYLAND_DISPLAY（剪贴板检查是 Wayland 专用）" >&2
  exit 77
fi
for tool in wl-copy wl-paste; do
  command -v "$tool" >/dev/null || { echo "跳过：没装 wl-clipboard（apt install wl-clipboard）" >&2; exit 77; }
done

BIN="$ROOT/target/debug/examples/clipboard"
cargo build --quiet -p nousdesk --example clipboard

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/home" "$TMP/config" "$TMP/data"

# 哨兵带上中文和时间戳：既是「mime 类型没截断中文」的证据，也避免读到上一轮的值。
SENTINEL="剪贴板检查 $(date +%H:%M:%S) 中文"
printf '%s' "$SENTINEL" | wl-copy

HOME="$TMP/home" XDG_CONFIG_HOME="$TMP/config" XDG_DATA_HOME="$TMP/data" \
  timeout 8 "$BIN" >"$TMP/out.txt" 2>&1 || true

fail=0
check() {
  local what="$1" needle="$2"
  if grep -Fq "$needle" "$TMP/out.txt"; then
    printf '  ✅ %s\n' "$what"
  else
    printf '  ❌ %s（没看到 %s）\n' "$what" "$needle"
    fail=1
  fi
}

echo "剪贴板检查（WAYLAND_DISPLAY=$WAYLAND_DISPLAY）："
check "读到别的程序写进去的内容" "READ1: Some(\"$SENTINEL\")"
check "粘贴动作把内容插进了输入框" "TEXT: \"$SENTINEL\""

if [ "$fail" != 0 ]; then
  # trap 会把临时目录删掉，先把日志留一份出来再退出。
  cp "$TMP/out.txt" /tmp/nousdesk-check-clipboard.log
  echo "失败：完整日志留在 /tmp/nousdesk-check-clipboard.log" >&2
  exit 1
fi
echo "通过：读 + 粘贴没问题。写（Ctrl+C → wl-paste）要人工验，见 AGENTS.md。"
