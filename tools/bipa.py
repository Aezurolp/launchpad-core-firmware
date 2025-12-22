import sys
import zlib
import argparse
from typing import List, Optional, Dict
import bipa_pb2

def get_diff_naive(source: bytes, target: bytes) -> list[tuple[int, bytes]]:
    inserts: list[tuple[int, bytes]] = []
    s_len = len(source)
    t_len = len(target)
    i = 0
    start: Optional[int] = None
    while i < max(s_len, t_len):
        s = source[i] if i < s_len else None
        t = target[i] if i < t_len else None
        if s != t:
            if start is None:
                start = i
        else:
            if start is not None:
                inserts.append((start, target[start:i]))
                start = None
        i += 1
    if start is not None:
        end = i if i <= t_len else t_len
        inserts.append((start, target[start:end]))
    return inserts


def get_diff_rolling(source: bytes, target: bytes, window: int = 32) -> list[tuple[int, bytes]]:
    if window <= 0:
        raise ValueError("window must be > 0")
    inserts: list[tuple[int, bytes]] = []
    s_len = len(source)
    t_len = len(target)
    index: Dict[int, List[int]] = {}
    if s_len >= window:
        for i in range(s_len - window + 1):
            h = zlib.adler32(source[i:i+window])
            index.setdefault(h, []).append(i)
    s_pos = 0
    t_pos = 0
    pending = bytearray()
    pending_start: Optional[int] = None
    while s_pos < s_len and t_pos < t_len and source[s_pos] == target[t_pos]:
        s_pos += 1
        t_pos += 1
    while t_pos < t_len:
        if s_pos < s_len and source[s_pos] == target[t_pos]:
            if pending:
                inserts.append((pending_start if pending_start is not None else t_pos - len(pending), bytes(pending)))
                pending.clear()
                pending_start = None
            s_pos += 1
            t_pos += 1
            continue
        if not pending:
            pending_start = t_pos
        pending.append(target[t_pos])
        anchored = False
        if t_pos + 1 >= window and s_len >= window:
            start = t_pos + 1 - window
            t_chunk = target[start:start+window]
            h = zlib.adler32(t_chunk)
            for cand in index.get(h, []):
                if source[cand:cand+window] == t_chunk:
                    inserts.append((pending_start if pending_start is not None else start, bytes(pending)))
                    pending.clear()
                    pending_start = None
                    s_pos = cand + window
                    t_pos = start + window
                    anchored = True
                    break
        if not anchored:
            if s_pos < s_len:
                s_pos += 1
            t_pos += 1
    if pending:
        inserts.append((pending_start if pending_start is not None else t_len - len(pending), bytes(pending)))
    return inserts

def create(source: str, target: str):
    patch = bipa_pb2.Patch()
    patch.version = 1

    source_data: bytes
    with open(source, "rb") as f:
        source_data = f.read()
        patch.checksum = str(zlib.crc32(source_data))

    target_data: bytes
    with open(target, "rb") as f:
        target_data = f.read()

    if len(source_data) == len(target_data):
        diffs = get_diff_naive(source_data, target_data)
    else:
        diffs = get_diff_rolling(source_data, target_data)
        patch.version = 2
    for offset, insert_bytes in diffs:
        insert = patch.inserts.add()
        insert.position = offset
        insert.data = insert_bytes

    with open(f"{target}.bipa", "wb") as f:
        f.write(patch.SerializeToString())

def patch(source: str, patch_file: str):
    with open(source, "rb") as f:
        source_data = f.read()

    patch_msg = bipa_pb2.Patch()
    with open(patch_file, "rb") as f:
        patch_msg.ParseFromString(f.read())

    checksum = str(zlib.crc32(source_data))
    if patch_msg.checksum and patch_msg.checksum != checksum:
        raise ValueError("Source file checksum does not match patch expectation")

    inserts = sorted(list(patch_msg.inserts), key=lambda ins: ins.position)

    # When version==2 (created for differing sizes), apply hunks as pure inserts.
    replace_mode = (patch_msg.version == 1) and all((ins.position + len(ins.data)) <= len(source_data) for ins in inserts)

    out = bytearray()
    src_pos = 0
    out_pos = 0

    for ins in inserts:
        pos = ins.position
        data = ins.data

        if pos < out_pos:
            raise ValueError(f"Patch hunks overlap or are out of order at position {pos}")

        while out_pos < pos and src_pos < len(source_data):
            out.append(source_data[src_pos])
            src_pos += 1
            out_pos += 1

        out.extend(data)
        out_pos += len(data)
        if replace_mode:
            src_pos += len(data)

    if src_pos < len(source_data):
        out.extend(source_data[src_pos:])

    if patch_file.endswith(".bipa"):
        out_path = patch_file[:-5]
    else:
        out_path = f"{source}.patched"

    with open(out_path, "wb") as f:
        f.write(out)

def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="bipa", description="BIPA — Binary Patching Toolkit CLI")

    subparsers = parser.add_subparsers(dest="command", required=True)

    create_parser = subparsers.add_parser("create", help="Create a .bipa patch from source and target",)
    create_parser.add_argument("--source", required=True, help="Path to source binary",)
    create_parser.add_argument("--target", required=True, help="Path to target binary",)

    patch_parser = subparsers.add_parser("patch", help="Apply a .bipa patch to a source binary")
    patch_parser.add_argument("--source", required=True, help="Path to source binary")
    patch_parser.add_argument("--patch", required=True, help="Path to .bipa patch file")

    return parser


def main(argv: Optional[List[str]] = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)

    if args.command == "create":
        create(args.source, args.target)
        return 0
    
    elif args.command == "patch":
        patch(args.source, args.patch)
        return 0

    parser.print_help()
    return 2

if __name__ == "__main__":
    sys.exit(main())
