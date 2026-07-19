from __future__ import annotations

import argparse
import struct
from pathlib import Path


UF2_MAGIC_START0 = 0x0A324655
UF2_MAGIC_START1 = 0x9E5D5157
UF2_MAGIC_END = 0x0AB16F30
UF2_FLAG_FAMILY_ID_PRESENT = 0x00002000
UF2_BLOCK_SIZE = 512
UF2_PAYLOAD_SIZE = 256
UF2_DATA_SIZE = 476

MYSTRIX_PRO_FAMILY_ID = 0xC47E5767

def parse_int(value: str) -> int:
    return int(value, 0)


def convert(input_path: Path, output_path: Path, base_address: int, family_id: int) -> int:
    data = input_path.read_bytes()
    if not data:
        raise ValueError("input binary is empty")
    if data[0] != 0xE9:
        raise ValueError("input is not an ESP application image (missing 0xe9 image magic)")
    if (
        len(data) > 0x10000
        and data[0x8000:0x8002] == b"\xaa\x50"
        and data[0x10000] == 0xE9
    ):
        raise ValueError(
            "input is a merged full-flash image; the Mystrix UF2 bootloader "
            "requires an application-only image"
        )

    block_count = (len(data) + UF2_PAYLOAD_SIZE - 1) // UF2_PAYLOAD_SIZE
    output_path.parent.mkdir(parents=True, exist_ok=True)

    with output_path.open("wb") as output:
        for block_no in range(block_count):
            offset = block_no * UF2_PAYLOAD_SIZE
            payload = data[offset : offset + UF2_PAYLOAD_SIZE]
            payload = payload.ljust(UF2_PAYLOAD_SIZE, b"\0")
            header = struct.pack(
                "<IIIIIIII",
                UF2_MAGIC_START0,
                UF2_MAGIC_START1,
                UF2_FLAG_FAMILY_ID_PRESENT,
                base_address + offset,
                UF2_PAYLOAD_SIZE,
                block_no,
                block_count,
                family_id,
            )
            block = header + payload + bytes(UF2_DATA_SIZE - UF2_PAYLOAD_SIZE)
            block += struct.pack("<I", UF2_MAGIC_END)
            assert len(block) == UF2_BLOCK_SIZE
            output.write(block)

    return block_count


def main() -> None:
    parser = argparse.ArgumentParser(description="Convert a binary to a UF2 image")
    parser.add_argument("input", type=Path, help="ESP32-S3 application binary")
    parser.add_argument("-o", "--output", required=True, type=Path, help="UF2 output path")
    parser.add_argument(
        "-b",
        "--base-address",
        type=parse_int,
        default=0,
        help="first flash address (default: 0x0, as used by MatrixOS)",
    )
    parser.add_argument(
        "-f",
        "--family-id",
        type=parse_int,
        default=MYSTRIX_PRO_FAMILY_ID,
        help="UF2 family ID (default: MatrixOS Mystrix1 ID 0xc47e5767)",
    )
    args = parser.parse_args()

    if (
        args.base_address < 0
        or args.base_address > 0xFFFFFFFF
        or args.family_id < 0
        or args.family_id > 0xFFFFFFFF
    ):
        parser.error("base address and family ID must fit in an unsigned 32-bit value")

    blocks = convert(args.input, args.output, args.base_address, args.family_id)
    print(f"wrote {args.output} ({blocks} UF2 blocks)")


if __name__ == "__main__":
    main()
