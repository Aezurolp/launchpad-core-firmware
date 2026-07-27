#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-only
# Copyright (C) 2026 ZephyrCodesStuff + Gemini 3.6 Flash

import sys
import os
import struct
import re

def encode_vlq(val):
    buf = bytearray()
    buf.append(val & 0x7F)
    val >>= 7
    while val > 0:
        buf.append((val & 0x7F) | 0x80)
        val >>= 7
    return bytes(reversed(buf))

def read_vlq(data, offset):
    val = 0
    while offset < len(data):
        byte = data[offset]
        offset += 1
        val = (val << 7) | (byte & 0x7F)
        if not (byte & 0x80):
            break
    return val, offset

def parse_boot_rs(content):
    end_tick_m = re.search(r'end_tick:\s*(\d+)', content)
    end_tick = int(end_tick_m.group(1)) if end_tick_m else 0

    frame_matches = re.findall(r'BootFrame\s*\{[^}]*tick:\s*(\d+)[^}]*count:\s*(\d+)[^}]*\}', content)
    frames = [(int(t), int(c)) for t, c in frame_matches]

    change_matches = re.findall(r'BootChange\s*\{[^}]*led:\s*(\d+)[^}]*velocity:\s*(\d+)[^}]*\}', content)
    changes = [(int(l), int(v)) for l, v in change_matches]

    return end_tick, frames, changes

def parse_bin(bin_path):
    with open(bin_path, 'rb') as f:
        data = f.read()

    end_tick, num_frames, num_changes = struct.unpack('<HHI', data[:8])
    offset = 8

    frames = []
    for _ in range(num_frames):
        tick, count, _pad = struct.unpack('<HBB', data[offset:offset+4])
        frames.append((tick, count))
        offset += 4

    changes = []
    for _ in range(num_changes):
        led, vel = struct.unpack('<BB', data[offset:offset+2])
        changes.append((led, vel))
        offset += 2

    return end_tick, frames, changes

def parse_midi(midi_path):
    with open(midi_path, 'rb') as f:
        data = f.read()

    if data[:4] != b'MThd':
        raise ValueError("Invalid MIDI header signature")

    hdr_len, fmt, ntracks, division = struct.unpack('>IHHH', data[4:14])
    offset = 14

    events_by_tick = {}
    max_tick = 0

    for _ in range(ntracks):
        if offset >= len(data):
            break
        trk_magic, trk_len = struct.unpack('>4sI', data[offset:offset+8])
        offset += 8
        trk_end = offset + trk_len

        abs_tick = 0
        running_status = None

        while offset < trk_end and offset < len(data):
            delta, offset = read_vlq(data, offset)
            abs_tick += delta
            if abs_tick > max_tick:
                max_tick = abs_tick

            status_byte = data[offset]
            if status_byte & 0x80:
                running_status = status_byte
                offset += 1
            else:
                status_byte = running_status

            if status_byte is None:
                break

            event_type = status_byte & 0xF0
            if event_type in (0x80, 0x90):
                note = data[offset]
                vel = data[offset+1]
                offset += 2
                if event_type == 0x80:
                    vel = 0
                if abs_tick not in events_by_tick:
                    events_by_tick[abs_tick] = []
                events_by_tick[abs_tick].append((note, vel))
            elif event_type in (0xA0, 0xB0, 0xE0):
                offset += 2
            elif event_type in (0xC0, 0xD0):
                offset += 1
            elif status_byte == 0xFF:
                _meta_type = data[offset]
                offset += 1
                length, offset = read_vlq(data, offset)
                offset += length
            elif status_byte in (0xF0, 0xF7):
                length, offset = read_vlq(data, offset)
                offset += length

    sorted_ticks = sorted(events_by_tick.keys())
    frames = []
    changes = []

    for t in sorted_ticks:
        evs = events_by_tick[t]
        frames.append((t, len(evs)))
        for note, vel in evs:
            changes.append((note, vel))

    return max_tick, frames, changes

def pack_bin(end_tick, frames, changes):
    buf = bytearray()
    buf.extend(struct.pack('<HHI', end_tick, len(frames), len(changes)))

    for tick, count in frames:
        buf.extend(struct.pack('<HBB', tick, count, 0))

    for led, vel in changes:
        buf.extend(struct.pack('<BB', led, vel))

    return buf

def unpack_midi(end_tick, frames, changes):
    track_bytes = bytearray()

    # Meta message: Set Tempo 120 BPM (500,000 us/beat)
    track_bytes.extend(encode_vlq(0))
    track_bytes.extend(b'\xFF\x51\x03\x07\xA1\x20')

    last_tick = 0
    change_idx = 0

    for frame_tick, count in frames:
        delta_10ms = frame_tick - last_tick

        for i in range(count):
            if change_idx >= len(changes):
                break
            led, vel = changes[change_idx]
            change_idx += 1

            dt = delta_10ms if i == 0 else 0
            track_bytes.extend(encode_vlq(dt))

            if vel > 0:
                track_bytes.extend(bytes([0x90, led & 0x7F, vel & 0x7F]))
            else:
                track_bytes.extend(bytes([0x80, led & 0x7F, 0x00]))

        last_tick = frame_tick

    # End of Track
    track_bytes.extend(encode_vlq(0))
    track_bytes.extend(b'\xFF\x2F\x00')

    # Build Header Chunk (MThd) & Track Chunk (MTrk)
    mid_file = bytearray()
    mid_file.extend(b'MThd')
    mid_file.extend(struct.pack('>IHHH', 6, 0, 1, 100))
    mid_file.extend(b'MTrk')
    mid_file.extend(struct.pack('>I', len(track_bytes)))
    mid_file.extend(track_bytes)

    return mid_file

def print_usage():
    print("Launchpad Boot Animation Converter Tool")
    print("Usage:")
    print("  python3 tools/boot_anim.py pack <input.mid|input.rs> <output.bin>")
    print("  python3 tools/boot_anim.py unpack <input.bin|input.rs> <output.mid>")
    print("  python3 tools/boot_anim.py convert <input_file> <output_file>")

def main():
    if len(sys.argv) < 3:
        print_usage()
        sys.exit(1)

    cmd = sys.argv[1]
    if cmd in ['pack', 'unpack']:
        if len(sys.argv) < 4:
            print_usage()
            sys.exit(1)
        input_path = sys.argv[2]
        output_path = sys.argv[3]
    elif cmd == 'convert':
        if len(sys.argv) < 4:
            print_usage()
            sys.exit(1)
        input_path = sys.argv[2]
        output_path = sys.argv[3]
        if output_path.endswith(('.mid', '.midi')):
            cmd = 'unpack'
        else:
            cmd = 'pack'
    else:
        input_path = sys.argv[1]
        output_path = sys.argv[2]
        cmd = 'unpack' if output_path.endswith(('.mid', '.midi')) else 'pack'

    if input_path.endswith(('.mid', '.midi')):
        end_tick, frames, changes = parse_midi(input_path)
    elif input_path.endswith('.bin'):
        end_tick, frames, changes = parse_bin(input_path)
    else:
        with open(input_path, 'r') as f:
            content = f.read()
        end_tick, frames, changes = parse_boot_rs(content)

    print(f"Animation Data: end_tick={end_tick}, frames={len(frames)}, changes={len(changes)}")

    os.makedirs(os.path.dirname(os.path.abspath(output_path)), exist_ok=True)

    if cmd == 'unpack':
        midi_data = unpack_midi(end_tick, frames, changes)
        with open(output_path, 'wb') as f:
            f.write(midi_data)
        print(f"Successfully unpacked {input_path} -> {output_path} ({len(midi_data)} bytes)")
    else:
        bin_data = pack_bin(end_tick, frames, changes)
        with open(output_path, 'wb') as f:
            f.write(bin_data)
        print(f"Successfully packed {input_path} -> {output_path} ({len(bin_data)} bytes)")

if __name__ == '__main__':
    main()
