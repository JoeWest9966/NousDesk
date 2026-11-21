#!/usr/bin/env bash
# 一个地址 + capabilities 的自动验收：真 nous-app（faux / 回环 / 临时库）+ desk_probe。
# 0 通过；1 失败；77 跳过（没有 nous-app 二进制）。
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NOUS_BIN="${NOUS_APP_BIN:-${CARGO_TARGET_DIR:-./target}/debug/nous-app}"
if [[ ! -x "$NOUS_BIN" ]]; then
  echo "SKIP: 没有 $NOUS_BIN（先在 Nous 仓里 cargo build -p nous-server --bin nous-app；或用 NOUS_APP_BIN 指定）"
  exit 77
fi

WORK="$(mktemp -d /tmp/desk-check.XXXXXX)"
SERVER_PID=""
cleanup() {
  if [[ -n "$SERVER_PID" ]]; then kill "$SERVER_PID" 2>/dev/null || true; wait "$SERVER_PID" 2>/dev/null || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT

TOKEN="probe-token-$(date +%s)-$$"
# 这个文件是给**引擎**读的，不是给 Desk 的：nous-app 的默认 token 路径是
# `$HOME/.config/nous/token`（`auth::default_token_path()`），Desk 的兜底是
# `~/.config/nousdesk/token` —— 两个应用、两个目录。文件不写，服务端就是**无 token**
# 启动，于是负向哨兵里「错 token 必须 401」会 FAIL（实测踩过：删了这三行，哨兵立刻红）。
mkdir -p "$WORK/home/.config/nous"
printf '%s\n' "$TOKEN" > "$WORK/home/.config/nous/token"
chmod 600 "$WORK/home/.config/nous/token"

# 端口 0 由内核选；绑定行是 stdout 唯一一行，脚本靠它拿真实地址。
HOME="$WORK/home" NOUS_PROVIDER=faux NOUS_LISTEN=127.0.0.1:0 \
  NOUS_DB="$WORK/nous.db" NOUS_PLUGINS_DB="$WORK/nous-plugins.db" "$NOUS_BIN" > "$WORK/server.out" 2> "$WORK/server.err" &
SERVER_PID=$!

ADDR=""
for _ in $(seq 1 200); do
  ADDR="$(sed -n 's/^listening on //p' "$WORK/server.out" | head -1)"
  [[ -n "$ADDR" ]] && break
  if ! kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "FAIL: nous-app 没起来"; cat "$WORK/server.err"; exit 1
  fi
  sleep 0.05
done
if [[ -z "$ADDR" ]]; then echo "FAIL: 没等到 listening on"; cat "$WORK/server.err"; exit 1; fi
HOST="http://$ADDR"

# 临时 desktop.toml：只有 host + token（老 board 不写）。
cat > "$WORK/desktop.toml" <<EOF
host = "$HOST"
token = "$TOKEN"
EOF

run_probe() {
  local label="$1"; shift
  echo "── $label ──"
  # tee 打命中行；PIPESTATUS 保住 probe 的退出码。
  # HOME 只给服务端找 token 用；cargo / rustup 的工具链目录必须留在原处，
  # 否则 rustup 会在临时 HOME 下重装一整套 stable（离线机器上直接失败）。
  local rustup_home="${RUSTUP_HOME:-$HOME/.rustup}"
  local cargo_home="${CARGO_HOME:-$HOME/.cargo}"
  NOUS_DESKTOP_CONFIG="$WORK/desktop.toml" HOME="$WORK/home" \
    RUSTUP_HOME="$rustup_home" CARGO_HOME="$cargo_home" \
    cargo run --quiet --manifest-path "$ROOT/Cargo.toml" -p nous-engine --example desk_probe -- \
      "$HOST" "$TOKEN" "$@" 2>&1 | tee "$WORK/$label.log"
  return "${PIPESTATUS[0]}"
}

run_probe positive
grep -E '^ASSERT OK' "$WORK/positive.log"
grep -q '^DONE$' "$WORK/positive.log" || { echo "FAIL: positive 没有 DONE"; exit 1; }

# 置顶往返（#10）：这几条都得真的验到，别让 probe 少跑一段还全绿。
# 写进去 → 跳到第一行且 pinned=true → 重放同锚不落第二条（version 不涨）→ 取消回原位
# → 不存在的会话 404（不静默降级）。
for pin_assert in pin.changed pin.jumps_to_top pin.pinned_true pin.replay.version_stable \
                  pin.unpinned_restored pin.missing.404; do
  grep -q "^ASSERT OK $pin_assert" "$WORK/positive.log" \
    || { echo "FAIL: 置顶往返少了一条断言：$pin_assert"; exit 1; }
done

# #12 判据 4：stats 200 + totals 在场；archive 200 且带 gens 数组。
# archive 原本钉的是「未实现就说未实现」（404）；Nous#67 把存档做出来之后，
# 钉的东西反过来：**必须真能拉到** —— 哪天它悄悄回 404，这条先红。
for assert_name in stats.usage.200.totals archive.200.gens; do
  grep -q "^ASSERT OK $assert_name" "$WORK/positive.log" \
    || { echo "FAIL: 缺了判据 4 的断言：$assert_name"; exit 1; }
done

# 负向哨兵：错 token 必须 401，且会话 / entries 条数不变。
# 若这里居然全绿，说明 401 闸门失效 —— 把 sentinel 反过来验一次：
# 探针自己会把「没被挡住」报 FAIL，脚本跟着红。
run_probe negative --negative
grep -E '^ASSERT OK' "$WORK/negative.log"
grep -q '^DONE$' "$WORK/negative.log" || { echo "FAIL: negative 没有 DONE"; exit 1; }

echo "DONE"
