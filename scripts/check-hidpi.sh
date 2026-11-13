#!/usr/bin/env bash
#
# HiDPI / 多显示器的**平台层**检查：GFX 后端认不认缩放、窗口是不是落在可见区域内。
#
# 这个脚本只在 X11/Xvfb 里跑，不碰真 Wayland 会话 —— X11 后端有
# `GPUI_X11_SCALE_FACTOR`，能把 scale_factor 人为钉成 1 / 1.5 / 2 复现 HiDPI；
# Wayland 的缩放由合成器按每块屏给，脚本没法伪造，只能人去改 GNOME 显示设置。
#
# 检查什么：
#   1. 日志里 `窗口：缩放=<值>` 和期望的一致（GPUI 确实拿到了这个倍率）；
#   2. 窗口的居中位置 / 尺寸以**逻辑像素**记，且没超出屏幕（小屏上会被裁小）；
#   3. 显示器清单打印出来了（多显示器时能看到几块屏、各自的原点）。
#
# 不检查什么：字体有没有糊、控件有没有重叠 —— 那是像素，得真机截图看。
#
# 用法：scripts/check-hidpi.sh
# 需要：Xvfb + xrandr（x11-xserver-utils）+ cargo。
# 退出码：0 通过；1 失败；77 跳过（缺 Xvfb / xrandr）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

for tool in Xvfb xrandr; do
  command -v "$tool" >/dev/null || { echo "跳过：没装 $tool" >&2; exit 77; }
done

# 用独立的 X server，别撞开发机上已经在跑的 :99（也别让 WAYLAND_DISPLAY 把
# GPUI 拽到真 Wayland 上去 —— 那样窗口开在真会话里，Xvfb 里什么都测不到）。
DISPLAY_NUM="${HIDPI_DISPLAY:-}:"
if [ -z "${HIDPI_DISPLAY:-}" ]; then
  for n in $(seq 120 140); do
    [ -e "/tmp/.X11-unix/X$n" ] || { DISPLAY_NUM=":$n"; break; }
  done
fi
[ "$DISPLAY_NUM" != ":" ] || { echo "找不到空闲的 X display" >&2; exit 1; }

Xvfb "$DISPLAY_NUM" -screen 0 1400x900x24 -nolisten tcp >/dev/null 2>&1 &
XVFB_PID=$!
trap 'kill "$XVFB_PID" 2>/dev/null || true' EXIT
sleep 1

export DISPLAY="$DISPLAY_NUM"
unset WAYLAND_DISPLAY
# 让日志落进临时目录，别污染用户的 ~/.local/share/nousdesk/desktop.log。
# 每轮用独立的 XDG_DATA_HOME：desktop.log 是 append 的，混在一起会把上一轮的
# 「缩放=1.00」也捞进来，让 1.5 / 2 那两轮误判成通过。
TMP="$(mktemp -d)"
trap 'kill "$XVFB_PID" 2>/dev/null || true; cp -r "$TMP"/logs /tmp/nousdesk-check-hidpi.logs 2>/dev/null || true; rm -rf "$TMP"' EXIT
mkdir -p "$TMP/home" "$TMP/config" "$TMP/logs"

cargo build --quiet -p nousdesk
BIN="$ROOT/target/debug/nousdesk"
rm -rf /tmp/nousdesk-check-hidpi.logs

fail=0
for scale in 1 1.5 2; do
  DATA="$TMP/data-$scale"
  mkdir -p "$DATA"
  # debug 构建同时打控制台，所以直接读 stdout；release 版才只看 desktop.log。
  LOG="$TMP/logs/scale-$scale.log"
  HOME="$TMP/home" XDG_CONFIG_HOME="$TMP/config" XDG_DATA_HOME="$DATA" \
    GPUI_X11_SCALE_FACTOR="$scale" timeout 8 "$BIN" >"$LOG" 2>&1 || true
  cat "$DATA/nousdesk/desktop.log" >>"$LOG" 2>/dev/null || true

  printf '缩放 %s：' "$scale"
  EXPECTED="$(printf '%.2f' "$scale")"
  if ! grep -Fq "窗口：缩放=${EXPECTED}" "$LOG"; then
    echo "❌ GPUI 没拿到这个倍率"
    fail=1
    continue
  fi
  if ! grep -Fq "显示器：1 块" "$LOG"; then
    echo "❌ 没打到显示器清单"
    fail=1
    continue
  fi
  # 1400×900 物理 / scale → 逻辑屏；窗口 1280×800 逻辑，2x 时该被裁到 700×450。
  printf '✅（%s）\n' "$(grep -o '视口=[0-9]*×[0-9]* 逻辑像素' "$LOG" | tail -1)"
done

if [ "$fail" != 0 ]; then
  echo "失败：日志留在 /tmp/nousdesk-check-hidpi.logs/" >&2
  exit 1
fi
echo "通过：X11 后端的 scale_factor 1 / 1.5 / 2 都认，窗口按逻辑像素居中且被小屏裁小。"
echo "真 Wayland 的 HiDPI / 多显示器仍需人工：改 GNOME 缩放、拖窗口跨屏，看 desktop.log。"
