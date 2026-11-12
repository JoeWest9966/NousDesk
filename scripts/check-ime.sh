#!/usr/bin/env bash
#
# 验输入法**客户端这一侧**：GPUI 有没有向合成器申请 IME、有没有把光标矩形报上去。
#
# 验不了「拼音能不能打出汉字」—— 那要真人敲键盘（合成器不会让别的进程伪装成输入法）。
# 这一份只钉协议：`zwp_text_input_v3.enable()` / `set_cursor_rectangle()` / `commit()`。
# 少了 enable，输入法根本不认这个窗口；少了矩形，候选框会飘到左上角 —— 两个都不是崩溃，
# 只是「中文打不出来 / 候选框在奇怪的地方」，不专门去抓就永远发现不了。
#
# 用法：scripts/check-ime.sh            # 需要 WAYLAND_DISPLAY（GNOME/KDE 的桌面上直接跑）
#      WAYLAND_DISPLAY=wayland-0 scripts/check-ime.sh
#
# 环境变量：IME_PROBE_SECONDS（默认 8）
#
# 退出码：0 = 协议对了；1 = 缺请求；77 = 没有 Wayland（跳过，不算失败）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SECS="${IME_PROBE_SECONDS:-8}"

if [ -z "${WAYLAND_DISPLAY:-}" ]; then
  echo "跳过：没有 WAYLAND_DISPLAY（这是 Wayland 专用检查；X11 走 zed-xim，另说）" >&2
  exit 77
fi

BIN="$ROOT/target/debug/examples/ime"
cargo build --quiet -p nousdesk --example ime

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

# 干净的 HOME/XDG：别让开发机的配置影响这次检查。
mkdir -p "$TMP/home" "$TMP/config" "$TMP/data"
HOME="$TMP/home" XDG_CONFIG_HOME="$TMP/config" XDG_DATA_HOME="$TMP/data" \
  WAYLAND_DEBUG=1 timeout "$SECS" "$BIN" >"$TMP/wl.log" 2>&1 || true

fail=0
check() {
  local what="$1" pattern="$2"
  if grep -Eq "$pattern" "$TMP/wl.log"; then
    printf '  ✅ %s\n' "$what"
  else
    printf '  ❌ %s（日志里没有 %s）\n' "$what" "$pattern"
    fail=1
  fi
}

echo "输入法协议检查（WAYLAND_DISPLAY=$WAYLAND_DISPLAY，$SECS 秒）："
check "窗口申请了 IME（text_input_v3.enable）" 'zwp_text_input_v3#[0-9]+\.enable\(\)'
check "报了光标矩形（候选框才贴得住光标）" 'zwp_text_input_v3#[0-9]+\.set_cursor_rectangle\('
check "提交了 IME 状态（enable + 矩形要 commit 才生效）" 'zwp_text_input_v3#[0-9]+\.commit\(\)'

# 光标矩形的尺寸读出来看一眼：宽 0 是合法的（那是插入符），高度 0 就不是了。
rect="$(grep -Eo 'set_cursor_rectangle\([0-9-]+, [0-9-]+, [0-9]+, [0-9]+\)' "$TMP/wl.log" | head -1 || true)"
if [ -n "$rect" ]; then
  echo "  光标矩形：$rect（x, y, 宽, 高）"
  h="$(printf '%s' "$rect" | sed -E 's/.*, ([0-9]+)\)/\1/')"
  if [ "$h" = "0" ]; then
    echo "  ❌ 高度是 0：输入法会把候选框放到看不见的地方" >&2
    fail=1
  fi
fi

if [ "$fail" != 0 ]; then
  echo "失败：IME 客户端协议不完整 —— 用 WAYLAND_DEBUG=1 cargo run --example ime 看完整日志" >&2
  exit 1
fi
echo "通过：客户端这一侧没问题。剩下的上屏效果要真人敲（见 AGENTS.md「中文输入法」）。"
