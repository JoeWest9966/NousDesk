#!/usr/bin/env python3
"""生成随包提示音（`assets/sounds/*.wav`）。

**为什么要一个脚本，而不是直接丢四个二进制进仓库**：音色会被反复调（音高、长短、
泛音量），一个能重跑、参数写在明处的生成器比四个来历不明的 wav 好维护 —— 想换音色
就改下面的参数重跑，不用重新找素材、也不用引一个音频编辑软件。

音型定为 **B · 二音上行**（`decisions.md` 2026-11-16）：
上行 = 完成，下行 = 失败。两个音都是 E5 起（659.26 Hz），第二个音 B5（987.77）或 A4（440）。

输出是 **44.1kHz / 单声道 / 16bit WAV**，不压缩：`paplay` / `pw-play` / `aplay`
三个播放器都原生吃 wav，不用为 60 KB 的东西引一个 vorbis 编解码。

用法：
    python3 scripts/gen-sounds.py          # 重新生成 assets/sounds/
"""
import math
import pathlib
import struct
import wave

RATE = 44_100
AUDIO_DIR = pathlib.Path(__file__).resolve().parent.parent / "assets" / "sounds"

E5, B5, A4, G4 = 659.26, 987.77, 440.00, 392.00


def silence(seconds: float) -> list[float]:
    return [0.0] * int(RATE * seconds)


def place(buf: list[float], at: float, samples: list[float]) -> None:
    """把一段波形叠加到 `at` 秒处，超出缓冲就自动扩。"""
    start = int(RATE * at)
    need = start + len(samples)
    if need > len(buf):
        buf.extend([0.0] * (need - len(buf)))
    for i, s in enumerate(samples):
        buf[start + i] += s


def bell(freq: float, dur: float, amp: float = 1.0, tau_scale: float = 1.0) -> list[float]:
    """一次敲击：基频 + 二/三次泛音，指数衰减。

    - 4 ms 起音：直接切正弦会在开头留一个咔哒声（那是阶跃，不是音）。
    - 泛音比基频衰减快，才有"木头/马林巴"那种一敲就散的听感；
      如果泛音和基频一样长，听起来像风琴。
    """
    n = int(RATE * dur)
    tau = dur / 4.0 * tau_scale
    out = []
    for i in range(n):
        t = i / RATE
        env = math.exp(-t / tau)
        if t < 0.004:
            env *= t / 0.004
        s = math.sin(2 * math.pi * freq * t)
        s += 0.25 * math.exp(-t / (tau * 0.55)) * math.sin(2 * math.pi * freq * 2 * t)
        s += 0.07 * math.exp(-t / (tau * 0.35)) * math.sin(2 * math.pi * freq * 3 * t)
        out.append(s * env * amp)
    return out


def sweep(f0: float, f1: float, dur: float, amp: float = 1.0) -> list[float]:
    """一段滑音。相位要累加，直接 `sin(2πf(t)t)` 会在中间出现一个假的倍频。"""
    n = int(RATE * dur)
    out, phase = [], 0.0
    for i in range(n):
        t = i / RATE
        k = t / dur
        f = f0 * (f1 / f0) ** k
        phase += 2 * math.pi * f / RATE
        env = math.exp(-t / (dur / 3.0))
        if t < 0.003:
            env *= t / 0.003
        out.append(math.sin(phase) * env * amp)
    return out


def hiss(dur: float, amp: float = 1.0) -> list[float]:
    """一撮噪声做"唰"。种子钉死，重跑出来的文件要逐字节一样。"""
    import random

    rng = random.Random(20261116)
    n = int(RATE * dur)
    out, prev = [], 0.0
    for i in range(n):
        t = i / RATE
        # 一阶高通：噪声直接听过低，听着像"沙"，滤掉低频才是"气"
        x = rng.uniform(-1.0, 1.0)
        y = x - prev
        prev = x
        env = (1 - i / n) ** 4
        if t < 0.002:
            env *= t / 0.002
        out.append(y * env * amp)
    return out


def normalize(buf: list[float], peak: float = 0.72) -> list[float]:
    top = max((abs(s) for s in buf), default=0.0)
    if top == 0:
        return buf
    k = peak / top
    return [s * k for s in buf]


def write_wav(name: str, buf: list[float]) -> pathlib.Path:
    AUDIO_DIR.mkdir(parents=True, exist_ok=True)
    path = AUDIO_DIR / name
    # 尾巴留 30 ms 静音：有些播放器在最后一个样本上会补一下，听着像第二声。
    buf = buf + silence(0.03)
    fade = int(RATE * 0.008)
    for i in range(min(fade, len(buf))):
        buf[len(buf) - 1 - i] *= i / fade
    frames = b"".join(struct.pack("<h", max(-32768, min(32767, int(s * 32767)))) for s in buf)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(frames)
    return path


def cue_done() -> list[float]:
    buf = silence(0.02)
    place(buf, 0.02, bell(E5, 0.13, amp=0.85))
    place(buf, 0.12, bell(B5, 0.22, amp=1.0))
    return normalize(buf)


def cue_error() -> list[float]:
    buf = silence(0.02)
    place(buf, 0.02, bell(E5, 0.13, amp=0.9, tau_scale=1.15))
    place(buf, 0.12, bell(A4, 0.26, amp=1.0, tau_scale=1.25))
    return normalize(buf, peak=0.66)


def cue_send() -> list[float]:
    buf = silence(0.005)
    place(buf, 0.005, hiss(0.055, amp=0.55))
    place(buf, 0.005, sweep(1320, 660, 0.06, amp=0.35))
    return normalize(buf, peak=0.42)


def cue_stop() -> list[float]:
    buf = silence(0.005)
    place(buf, 0.005, sweep(G4, 262, 0.13, amp=1.0))
    return normalize(buf, peak=0.55)


def main() -> None:
    for name, fn in [
        ("done.wav", cue_done),
        ("error.wav", cue_error),
        ("send.wav", cue_send),
        ("stop.wav", cue_stop),
    ]:
        path = write_wav(name, fn())
        print(f"{path}  {path.stat().st_size / 1024:.1f} KB")


if __name__ == "__main__":
    main()
