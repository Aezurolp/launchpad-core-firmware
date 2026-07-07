#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2025-2026 Anthony Hofmeister


from __future__ import annotations

import argparse
import sys
import time
from pathlib import Path
from typing import List, Sequence


SYSEX_START = 0xF0
SYSEX_END = 0xF7


def load_mido():
    try:
        import mido  # type: ignore
    except ImportError as exc:
        raise SystemExit(
            "Missing dependency: install with "
            "`python3 -m pip install mido python-rtmidi`."
        ) from exc
    return mido


def parse_sysex_messages(raw: bytes) -> List[bytes]:
    messages: List[bytes] = []
    index = 0
    total = len(raw)

    while index < total:
        while index < total and raw[index] != SYSEX_START:
            index += 1
        if index >= total:
            break

        end = index + 1
        while end < total and raw[end] != SYSEX_END:
            end += 1
        if end >= total:
            raise ValueError("Unterminated SysEx message in input file.")

        messages.append(raw[index:end + 1])
        index = end + 1

    if not messages:
        raise ValueError("No SysEx messages found in input file.")

    return messages


def list_output_ports(mido) -> List[str]:
    ports = list(mido.get_output_names())
    if not ports:
        raise RuntimeError("No MIDI output ports available.")
    return ports


def print_ports(ports: Sequence[str]) -> None:
    print("Available MIDI output ports:")
    for index, name in enumerate(ports, start=1):
        print(f"{index}. {name}")


def select_output_port(ports: Sequence[str]) -> str:
    print_ports(ports)
    prompt = f"Select MIDI output [1-{len(ports)}]: "

    while True:
        choice = input(prompt).strip()
        try:
            selected = int(choice)
        except ValueError:
            print("Please enter a number.")
            continue

        if 1 <= selected <= len(ports):
            return ports[selected - 1]

        print(f"Please choose a value from 1 to {len(ports)}.")


def send_sysex_file(mido, syx_path: Path, output_name: str, delay_ms: float) -> None:
    messages = parse_sysex_messages(syx_path.read_bytes())
    delay_seconds = max(delay_ms, 0.0) / 1000.0
    start_time = time.monotonic()
    total_bytes = 0

    with mido.open_output(output_name) as port:
        print(f"Opened MIDI output: {output_name}")
        print(f"Sending {len(messages)} SysEx message(s) from {syx_path}...")

        for index, message in enumerate(messages, start=1):
            port.send(mido.Message("sysex", data=message[1:-1]))
            total_bytes += len(message)

            # Premium dynamic progress bar to reduce stdout IO bottleneck
            pct = index / len(messages)
            bar_len = 30
            filled = int(bar_len * pct)
            bar = "█" * filled + "░" * (bar_len - filled)
            elapsed = time.monotonic() - start_time
            speed = index / elapsed if elapsed > 0 else 0

            sys.stdout.write(
                f"\r\033[35m\033[1mFlashing:\033[0m \033[36m[{bar}]\033[0m "
                f"{pct*100:5.1f}% ({index}/{len(messages)} msgs) | "
                f"{speed:.1f} msgs/s | {elapsed:.1f}s elapsed"
            )
            sys.stdout.flush()

            if delay_seconds and index != len(messages):
                time.sleep(delay_seconds)

    print()
    print(f"\033[32m\033[1mSuccess! Sent {total_bytes} bytes in {time.monotonic() - start_time:.2f}s.\033[0m")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Send a .syx firmware file to a chosen MIDI output port."
    )
    parser.add_argument(
        "syx_path",
        nargs="?",
        help="Path to the .syx file to send.",
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="List available MIDI output ports and exit.",
    )
    parser.add_argument(
        "--delay-ms",
        type=float,
        default=1.0,
        help="Delay between SysEx messages in milliseconds (default: 1.0).",
    )
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()

    mido = load_mido()
    ports = list_output_ports(mido)

    if args.list:
        print_ports(ports)
        return 0

    if not args.syx_path:
        parser.error("the following argument is required: syx_path")

    syx_path = Path(args.syx_path)
    if not syx_path.is_file():
        raise SystemExit(f"Input file not found: {syx_path}")

    output_name = select_output_port(ports)
    send_sysex_file(mido, syx_path, output_name, args.delay_ms)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except KeyboardInterrupt:
        raise SystemExit("\nCancelled.")
