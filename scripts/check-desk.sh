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

# Desk#11：引擎 PresetStore 的根来自 NOUS_PRESETS。预置一条 roster 条目，
# 让 `GET /v1/presets/Session-Template` 返回 200 且 items 非空（判据 4）。
mkdir -p "$WORK/presets/Session-Template/probe"
# 形状要过 `nous-presets-admin` 的校验：`v` 必须是 1、`name` 等于目录名、`summary`
# 非空，且键只能是 PRESET_KEYS 里那 13 个（多一个 `spec` 就被判「不认识的键」，
# 那条 roster 断言从 2026-11-23 起一直是红的 —— 种子过期，不是端点坏了）。
printf '%s\n' '{"v":1,"name":"probe","summary":"探针模板","category":[]}' \
  > "$WORK/presets/Session-Template/probe/preset.json"

# 扩展页的三份数据要有东西可断言：临时源码根里放两条 —— 一条合法原生清单、
# 一条 TS 时代（`index.js` + 字符串 sections）的老清单。没有种子，`kind` 一栏
# 永远空着，`ext.items.kind.*` 两条断言就会变成「空也能过」。
mkdir -p "$WORK/extend/native-one" "$WORK/extend/old-js"
printf '%s\n' '{"name":"native-one","version":"0.1.0","protocol":1,"file":"native-one","entry":"native-one","sections":[{"key":"a","tag":"a"}],"summary":"探针种的原生扩展","category":"工具"}' \
  > "$WORK/extend/native-one/nous.json"
printf '%s\n' '{"name":"old-js","version":"0.1.0","file":"index.js","entry":"default","sections":["preamble"]}' \
  > "$WORK/extend/old-js/nous.json"

# 底部仓片（Desk#23）的三条判据要有真数据：把 extend 做成一个 git 仓 ——
# 分支 `feat/probe`（非 main）、**不加 remote**、脏。再放 2 处内容改动
# （dirty-a / dirty-b）+ 2 个仓级文件（artifacts/、deleted-extensions.json），
# 后两个必须被 `pending_unit_count` 排除，所以「处」= 2。
git -C "$WORK/extend" init -q
git -C "$WORK/extend" config user.email probe@nous.local
git -C "$WORK/extend" config user.name probe
git -C "$WORK/extend" config commit.gpgsign false
git -C "$WORK/extend" add -A
git -C "$WORK/extend" commit -qm seed
git -C "$WORK/extend" checkout -q -b feat/probe
mkdir -p "$WORK/extend/dirty-a" "$WORK/extend/dirty-b" "$WORK/extend/artifacts"
printf 'x\n' > "$WORK/extend/dirty-a/index.js"
printf 'x\n' > "$WORK/extend/dirty-b/index.js"
printf 'x\n' > "$WORK/extend/artifacts/keep.txt"
printf '%s\n' '[]' > "$WORK/extend/deleted-extensions.json"

# 元素库判据：删除必须**真的从私库盘上消失**。种子是一个临时 git 私库 ——
# 引擎写一次提交一次，仓不在或没配 user 就会 500，所以这里把 user 也配好。
mkdir -p "$WORK/elements/groups" "$WORK/elements/packages"
printf '%s\n' '{"id":"probe-group","name":"探针组","desc":"删掉我","packages":["probe-pack"]}' \
  > "$WORK/elements/groups/probe-group.json"
printf '%s\n' '{"id":"probe-pack","name":"探针包","kind":"prompt","body":"hi"}' \
  > "$WORK/elements/packages/probe-pack.json"
git -C "$WORK/elements" init -q
git -C "$WORK/elements" config user.email probe@nous.local
git -C "$WORK/elements" config user.name probe
# 这台机器可能开着全局 commit.gpgsign（CI / 开发机带签名钩子），临时私库签不了就
# 写不进提交 —— 这一步只影响临时库，不动任何人的全局配置。
git -C "$WORK/elements" config commit.gpgsign false
git -C "$WORK/elements" add -A
git -C "$WORK/elements" commit -qm seed

# 端口 0 由内核选；绑定行是 stdout 唯一一行，脚本靠它拿真实地址。
HOME="$WORK/home" NOUS_PROVIDER=faux NOUS_LISTEN=127.0.0.1:0 \
  NOUS_PRESETS="$WORK/presets" NOUS_DB="$WORK/nous.db" NOUS_PLUGINS_DB="$WORK/nous-plugins.db" \
  NOUS_EXTEND="$WORK/extend" NOUS_ELEMENTS="$WORK/elements" "$NOUS_BIN" > "$WORK/server.out" 2> "$WORK/server.err" &
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

run_probe positive --elements "$WORK/elements"
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
# 资源页（扩展段）的三份读面：清单分档、类别词表、归档区、变体表。
# 少一条就是「页面照常打开、那一行永远没有」——这种静默最费劲，所以钉死。
for assert_name in ext.items.kind.native ext.items.kind.legacy_js \
                  ext.categories.200 ext.archive.200 ext.variants.200 \
                  repo.status.branch.non_main repo.status.no_remote repo.pending.extend_count; do
  grep -q "^ASSERT OK $assert_name" "$WORK/positive.log" \
    || { echo "FAIL: 扩展段少了一条断言：$assert_name"; exit 1; }
done

for assert_name in stats.usage.200.totals archive.200.gens presets.enabled presets.roster.200 presets.roster.items.nonempty; do
  grep -q "^ASSERT OK $assert_name" "$WORK/positive.log" \
    || { echo "FAIL: 缺了判据 4 的断言：$assert_name"; exit 1; }
done

# 元素库删除：删除必须真的从盘上消失，且过期 `base` 不许动文件。
# 2026-11-24 真机那个「删不掉」的坑就在这条缝上：协议层说清 409 语义，
# 客户端才敢「409 → 重读 → 再点一次」。——少一条就是那段回归又能滑过去。
for assert_name in elements.list.etag elements.stale-base.409 elements.stale-base.kept \
                  elements.delete.commit elements.delete.file-gone elements.delete.listed-gone \
                  elements.delete.replay-no-second-write elements.delete.replay-still-gone \
                  elements.delete.nobase; do
  grep -q "^ASSERT OK $assert_name" "$WORK/positive.log" \
    || { echo "FAIL: 元素库删除少了一条断言：$assert_name"; exit 1; }
done

# 负向哨兵：错 token 必须 401，且会话 / entries 条数不变。
# 若这里居然全绿，说明 401 闸门失效 —— 把 sentinel 反过来验一次：
# 探针自己会把「没被挡住」报 FAIL，脚本跟着红。
run_probe negative --negative
grep -E '^ASSERT OK' "$WORK/negative.log"
grep -q '^DONE$' "$WORK/negative.log" || { echo "FAIL: negative 没有 DONE"; exit 1; }

echo "DONE"
