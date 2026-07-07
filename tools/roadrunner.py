#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2025-2026 Anthony Hofmeister

from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path
from typing import Dict, NamedTuple, Optional, Sequence, Tuple


REQ = [0x00, 0x20, 0x29, 0x02, 0x0E, 0x70]
RESP = [0x00, 0x20, 0x29, 0x02, 0x0E, 0x71]
FLASH_BASE = 0x08000000
CHUNK_LEN = 256

STATUS_NAMES: Dict[int, str] = {
    0x00: "OK",
    0x01: "INIT",
    0x02: "SYNC",
    0x03: "CMD",
    0x04: "NACK",
    0x05: "RX",
    0x06: "ARG",
}

FIRMWARE_NAMES: Dict[int, str] = {
    0x00: "legacy",
    0x01: "roadrunner",
    0x02: "unknown",
    0x03: "bootloader",
    0x7F: "error",
}


class Status(NamedTuple):
    status: int
    kind: int
    major: int
    minor: int
    patch: int
    probe_status: int
    read_status: int
    ack: int
    baud: int
    pid: int
    blid: int
    vector: bytes


class Stats(NamedTuple):
    status: int
    fast_frames: int
    commits: int
    rx_overruns: int


def load_mido():
    try:
        import mido  # type: ignore
    except ImportError as exc:
        raise SystemExit(
            "Missing dependency: install with `python3 -m pip install mido python-rtmidi`."
        ) from exc
    return mido


def resolve_port(ports: Sequence[str], selector: Optional[str]) -> Optional[str]:
    if selector is None:
        return None
    try:
        index = int(selector)
    except ValueError:
        index = 0
    if 1 <= index <= len(ports):
        return ports[index - 1]
    exact = [name for name in ports if name == selector]
    if exact:
        return exact[0]
    matches = [name for name in ports if selector.lower() in name.lower()]
    if len(matches) == 1:
        return matches[0]
    if len(matches) > 1:
        raise SystemExit(f"Ambiguous port selector {selector!r}: {matches}")
    raise SystemExit(f"Port not found: {selector}")


def choose_port(title: str, ports: Sequence[str]) -> str:
    if not ports:
        raise SystemExit(f"No {title.lower()} available.")
    print(title)
    for index, name in enumerate(ports, start=1):
        print(f"  {index:2d}. {name}")
    while True:
        choice = input(f"Select port [1-{len(ports)}]: ").strip()
        try:
            index = int(choice)
        except ValueError:
            print("Please enter a number.")
            continue
        if 1 <= index <= len(ports):
            return ports[index - 1]
        print(f"Please choose a value from 1 to {len(ports)}.")


def choose_ports(mido, out_selector: Optional[str], in_selector: Optional[str]) -> Tuple[str, str]:
    outputs = list(mido.get_output_names())
    inputs = list(mido.get_input_names())
    out_name = resolve_port(outputs, out_selector)
    if out_name is None:
        out_name = choose_port("Available MIDI output ports:", outputs)
    in_name = resolve_port(inputs, in_selector)
    if in_name is None:
        in_name = out_name if out_name in inputs else choose_port("Available MIDI input ports:", inputs)
    return out_name, in_name


def list_ports(mido) -> None:
    print("MIDI output ports:")
    for index, name in enumerate(mido.get_output_names(), start=1):
        print(f"  {index:2d}. {name}")
    print("MIDI input ports:")
    for index, name in enumerate(mido.get_input_names(), start=1):
        print(f"  {index:2d}. {name}")


def transact(mido, outport, inport, cmd: str, payload: bytes = b"", timeout: float = 5.0, retries: int = 2) -> bytes:
    expected = RESP + [ord(cmd)]
    last_error: Optional[BaseException] = None
    for attempt in range(retries + 1):
        try:
            for _ in inport.iter_pending():
                pass
            outport.send(mido.Message("sysex", data=REQ + [ord(cmd)] + list(payload)))
            deadline = time.monotonic() + timeout
            while time.monotonic() < deadline:
                for msg in inport.iter_pending():
                    if msg.type != "sysex":
                        continue
                    data = list(msg.data)
                    if data[: len(expected)] == expected:
                        return bytes(data[len(expected) :])
                time.sleep(0.002)
            raise TimeoutError(f"Timed out waiting for {cmd!r} response.")
        except TimeoutError as exc:
            last_error = exc
            if attempt < retries:
                time.sleep(0.05)
    if last_error is not None:
        raise last_error
    raise TimeoutError("Transaction timed out.")


def parse_simple(payload: bytes) -> int:
    text = payload.decode("ascii")
    if len(text) < 2:
        raise ValueError(f"Short response: {text!r}")
    return int(text[:2], 16)


def parse_chunk(payload: bytes) -> Tuple[int, int, int]:
    text = payload.decode("ascii")
    if len(text) < 14:
        raise ValueError(f"Short chunk response: {text!r}")
    return int(text[0:2], 16), int(text[2:10], 16), int(text[10:14], 16)


def parse_status(payload: bytes) -> Status:
    text = payload.decode("ascii")
    if len(text) < 36:
        raise ValueError(f"Short status response: {text!r}")
    status = int(text[0:2], 16)
    kind = int(text[2:4], 16)
    major = int(text[4:6], 16)
    minor = int(text[6:8], 16)
    patch = int(text[8:10], 16)
    probe_status = int(text[10:12], 16)
    read_status = int(text[12:14], 16)
    ack = int(text[14:16], 16)
    baud = int(text[16:24], 16)
    pid = int(text[24:28], 16)
    blid = int(text[28:30], 16)
    vector_len = int(text[30:32], 16)
    vector_hex = text[32 : 32 + vector_len * 2]
    vector = bytes.fromhex(vector_hex) if vector_hex else b""
    return Status(status, kind, major, minor, patch, probe_status, read_status, ack, baud, pid, blid, vector)


def parse_stats(payload: bytes) -> Stats:
    text = payload.decode("ascii")
    if len(text) < 26:
        raise ValueError(f"Short stats response: {text!r}")
    return Stats(
        int(text[0:2], 16),
        int(text[2:10], 16),
        int(text[10:18], 16),
        int(text[18:26], 16),
    )


def status_name(value: int) -> str:
    return STATUS_NAMES.get(value, f"UNKNOWN_{value:02X}")


def print_status(status: Status) -> None:
    name = FIRMWARE_NAMES.get(status.kind, f"unknown-{status.kind:02X}")
    version = f" {status.major}.{status.minor}.{status.patch}" if status.kind == 1 else ""
    print(f"M0 firmware: {name}{version}")
    print(f"Status:      0x{status.status:02X} ({status_name(status.status)})")
    print(f"ROM probe:   0x{status.probe_status:02X} ({status_name(status.probe_status)})")
    print(f"Read status: 0x{status.read_status:02X} ({status_name(status.read_status)})")
    print(f"ACK:         0x{status.ack:02X}")
    print(f"Baud:        {status.baud}")
    print(f"PID:         0x{status.pid:04X}")
    print(f"BLID:        0x{status.blid:02X}")
    if status.vector:
        print("Vector:      " + " ".join(f"{byte:02X}" for byte in status.vector))


def print_stats(stats: Stats) -> None:
    print(f"Status:      0x{stats.status:02X} ({status_name(stats.status)})")
    print(f"Fast frames: {stats.fast_frames}")
    print(f"Commits:     {stats.commits}")
    print(f"RX overruns: {stats.rx_overruns}")


def progress(prefix: str, done: int, total: int, start: float) -> None:
    width = 30
    filled = width if total == 0 else int(width * done / total)
    bar = "=" * filled + "-" * (width - filled)
    elapsed = max(time.monotonic() - start, 0.001)
    speed = done / elapsed / 1024
    sys.stdout.write(f"\r{prefix}: [{bar}] {done}/{total} B {speed:.1f} KB/s")
    sys.stdout.flush()


def do_status(mido, outport, inport, timeout: float) -> Status:
    status = parse_status(transact(mido, outport, inport, "S", timeout=timeout))
    print_status(status)
    return status


def do_cached_status(mido, outport, inport, timeout: float) -> Status:
    status = parse_status(transact(mido, outport, inport, "C", timeout=timeout))
    print_status(status)
    return status


def do_stats(mido, outport, inport, timeout: float) -> Stats:
    stats = parse_stats(transact(mido, outport, inport, "T", timeout=timeout))
    print_stats(stats)
    return stats


def do_flash(mido, outport, inport, timeout: float, path: Path, base: int) -> None:
    data = path.read_bytes()
    if not data:
        raise SystemExit("Binary is empty.")

    print(f"Flashing {path} ({len(data)} bytes) to 0x{base:08X}")
    begin = parse_simple(transact(mido, outport, inport, "B", f"{base:08X}{len(data):08X}".encode("ascii"), timeout=timeout))
    if begin != 0:
        raise SystemExit(f"Flash begin failed: 0x{begin:02X} ({status_name(begin)})")

    # Dynamically determine the maximum chunk size supported by the firmware.
    # We default to CHUNK_LEN (256) and fall back to 64 if we get an ARG error (0x06).
    chunk_len = CHUNK_LEN
    offset = 0
    start = time.monotonic()
    while offset < len(data):
        chunk = data[offset : offset + chunk_len]
        payload = f"{base + offset:08X}{len(chunk):04X}{chunk.hex().upper()}".encode("ascii")
        try:
            status, addr, written = parse_chunk(transact(mido, outport, inport, "D", payload, timeout=timeout))
        except TimeoutError as exc:
            raise SystemExit(f"\nWrite timed out at 0x{base + offset:08X}: {exc}")

        if status == 0x06 and chunk_len > 64:  # ARG error, could be that chunk size is too large
            # Fall back to 64-byte chunks and retry this block
            chunk_len = 64
            continue

        if status != 0 or addr != base + offset or written != len(chunk):
            raise SystemExit(f"\nWrite failed at 0x{base + offset:08X}: 0x{status:02X} ({status_name(status)})")

        progress("Writing", offset + len(chunk), len(data), start)
        offset += chunk_len
    print()

    offset = 0
    start = time.monotonic()
    while offset < len(data):
        chunk = data[offset : offset + chunk_len]
        payload = f"{base + offset:08X}{len(chunk):04X}{chunk.hex().upper()}".encode("ascii")
        status, addr, verified = parse_chunk(transact(mido, outport, inport, "V", payload, timeout=timeout))
        if status != 0 or addr != base + offset or verified != len(chunk):
            raise SystemExit(f"\nVerify failed at 0x{base + offset:08X}: 0x{status:02X} ({status_name(status)})")
        progress("Verifying", offset + len(chunk), len(data), start)
        offset += chunk_len
    print()

    print("Booting M0 and refreshing status...")
    final = parse_status(transact(mido, outport, inport, "O", timeout=max(timeout, 8.0)))
    print_status(final)
    if final.kind != 1:
        raise SystemExit("Flash verified, but Roadrunner did not answer the version inquiry.")


def main() -> None:
    parser = argparse.ArgumentParser(description="Launchpad Pro MK3 Roadrunner M0 tool")
    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--out-port", help="MIDI output port name, substring, or index")
    common.add_argument("--in-port", help="MIDI input port name, substring, or index")
    common.add_argument("--timeout", type=float, default=5.0)
    sub = parser.add_subparsers(dest="command", required=True)
    sub.add_parser("list-ports", help="List available MIDI input and output ports")
    sub.add_parser("status", parents=[common], help="Probe M0 firmware status")
    sub.add_parser(
        "cached-status",
        parents=[common],
        help="Read last known M0 firmware status without probing",
    )
    sub.add_parser("stats", parents=[common], help="Read Roadrunner throughput counters")
    flash = sub.add_parser("flash", parents=[common], help="Flash and verify Roadrunner firmware")
    flash.add_argument("binary", type=Path)
    flash.add_argument("--base", type=lambda value: int(value, 0), default=FLASH_BASE)
    args = parser.parse_args()

    mido = load_mido()
    if args.command == "list-ports":
        list_ports(mido)
        return

    out_name, in_name = choose_ports(mido, args.out_port, args.in_port)
    with mido.open_output(out_name) as outport, mido.open_input(in_name) as inport:
        print(f"Opened MIDI output: {out_name}")
        print(f"Opened MIDI input:  {in_name}")
        if args.command == "status":
            do_status(mido, outport, inport, args.timeout)
        elif args.command == "cached-status":
            do_cached_status(mido, outport, inport, args.timeout)
        elif args.command == "stats":
            do_stats(mido, outport, inport, args.timeout)
        elif args.command == "flash":
            do_flash(mido, outport, inport, args.timeout, args.binary, args.base)


if __name__ == "__main__":
    main()
