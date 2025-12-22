import sys
import os
import argparse
import struct
import math
from typing import List, Optional
from dataclasses import dataclass

SYSEX_START = 0xF0
SYSEX_END = 0xF7

NOVATION_HEADER = [0x00, 0x20, 0x29, 0x00]
RGB_FIRMWARE_FOOTER = list(b"Firmware")

UPDATE_INIT = 0x71
UPDATE_HEADER = 0x7C
UPDATE_WRITE = 0x72
UPDATE_FINISH = 0x73
UPDATE_FOOTER = 0x76

LPX_FAMILY_ID = 0x02
LPRGB_FAMILY_ID = 0x00

LPX_PRODUCT_ID = 0x0C
LPMINIMK3_PRODUCT_ID = 0x0D
LPPROMK3_PRODUCT_ID = 0x0E
LPMK2_PRODUCT_ID = 0x69
LPPRO_PRODUCT_ID = 0x51

BLOCK_SIZE_BITS = 0x100
BLOCK_SIZE_BYTES = math.ceil(BLOCK_SIZE_BITS / 8)
BLOCK_SIZE_7BITS = math.ceil(BLOCK_SIZE_BITS / 7)

@dataclass
class Product:
	flag: str
	family_id: int
	product_id: int
	name: str

PRODUCTS = [
	Product("/x", LPX_FAMILY_ID, LPX_PRODUCT_ID, "Launchpad X"),
	Product("/minimk3", LPX_FAMILY_ID, LPMINIMK3_PRODUCT_ID, "Launchpad Mini MK3"),
	Product("/lppmk3", LPX_FAMILY_ID, LPPROMK3_PRODUCT_ID, "Launchpad Pro MK3"),
	Product("/mk2", LPRGB_FAMILY_ID, LPMK2_PRODUCT_ID, "Launchpad MK2"),
	Product("/lpp", LPRGB_FAMILY_ID, LPPRO_PRODUCT_ID, "Launchpad Pro"),
]

class BinToSyx:
	def __init__(self, product: Product, version: str, input_file: str, output_file: Optional[str] = None):
		self.product = product
		self.version = self._parse_version(version)
		self.input_file = input_file
		self.output_file = output_file or self._generate_output_filename(input_file)
		self.input_data = bytearray()
		self.output_data = bytearray()

	def _parse_version(self, version: str) -> List[int]:
		if len(version) != 3:
			raise ValueError("Version should be 3 characters long")
		parsed = []
		for ch in version:
			if '0' <= ch <= '9':
				parsed.append(ord(ch) - ord('0'))
			else:
				up = ch.upper()
				if 'A' <= up <= 'F':
					parsed.append(ord(up) - ord('A') + 10)
				else:
					raise ValueError(f"Invalid version character: {ch}")
		return parsed

	def _generate_output_filename(self, input_file: str) -> str:
		base, _ = os.path.splitext(input_file)
		return f"{base}.syx"

	def _create_sysex_start(self, msg_type: int) -> bytearray:
		data = bytearray([SYSEX_START])
		data.extend(NOVATION_HEADER)
		data.append(msg_type)
		return data

	def _uint_to_nibbles(self, n: int) -> List[int]:
		out = []
		for _ in range(8):
			out.append((n & 0xF0000000) >> 28)
			n = ((n << 4) & 0xFFFFFFFF)
		return out

	def _crc32(self, data: bytearray) -> int:
		crc = 0xFFFFFFFF
		for b in data:
			crc ^= (b << 24) & 0xFFFFFFFF
			for _ in range(8):
				if crc & 0x80000000:
					crc = ((crc << 1) ^ 0x04C11DB7) & 0xFFFFFFFF
				else:
					crc = (crc << 1) & 0xFFFFFFFF
		return crc

	def _write_block(self, block_index: int, update_type: int) -> bytearray:
		data = self._create_sysex_start(update_type)
		output_bytes = bytearray(BLOCK_SIZE_7BITS)
		if self.product.product_id == LPPROMK3_PRODUCT_ID:
			for k in range(BLOCK_SIZE_BITS):
				shift = 7 - (k % 8)
				target_index = k // 7
				read_index = block_index * BLOCK_SIZE_BYTES + k // 8
				bit = 0
				if read_index < len(self.input_data):
					bit = (self.input_data[read_index] & (1 << shift)) >> shift
				if k % 7 == 0:
					output_bytes[target_index] = 0
				output_bytes[target_index] |= bit << (6 - (k % 7))
		else:
			for k in range(BLOCK_SIZE_BITS):
				shift = 7 - (k % 8)
				target_index = k // 7
				read_index = block_index * BLOCK_SIZE_BYTES + (k // 8)
				if read_index >= len(self.input_data):
					bit = 1
				else:
					bit = (self.input_data[read_index] >> shift) & 0x1
				if k % 7 == 0:
					output_bytes[target_index] = 0
				output_bytes[target_index] |= (bit << (6 - (k % 7)))
		data.extend(output_bytes)
		data.append(SYSEX_END)
		return data

	def convert(self):
		with open(self.input_file, "rb") as f:
			self.input_data = bytearray(f.read())
		if (self.product.family_id == LPX_FAMILY_ID and 
			self.product.product_id == LPPROMK3_PRODUCT_ID and 
			len(self.input_data) >= 8):
			internal_crc = self._crc32(self.input_data[:-8])
			self.input_data[-4:] = struct.pack('<I', internal_crc)
			print(f"Pro MK3: Patched internal CRC: 0x{internal_crc:08X}")
		blocks = math.ceil(len(self.input_data) / BLOCK_SIZE_BYTES)
		init_msg = self._create_sysex_start(UPDATE_INIT)
		init_msg.extend([
			self.product.family_id,
			self.product.product_id,
			0x00, 0x00, 0x00,
			self.version[0] & 0xF,
			self.version[1] & 0xF,
			self.version[2] & 0xF
		])
		init_msg.append(SYSEX_END)
		self.output_data.extend(init_msg)
		if self.product.family_id == LPX_FAMILY_ID:
			header = self._create_sysex_start(UPDATE_HEADER)
			header.append(1 if self.product.product_id == LPPROMK3_PRODUCT_ID else 0)
			header.extend([0x30, 0x30, 0x30])
			header.extend([0x30 | (v & 0xF) for v in self.version])
			header.extend(self._uint_to_nibbles(len(self.input_data)))
			crc = self._crc32(self.input_data)
			if self.product.product_id == LPPROMK3_PRODUCT_ID:
				print(f"CRC32 (SysEx header): 0x{crc:08X}")
			header.extend(self._uint_to_nibbles(crc))
			header.append(SYSEX_END)
			self.output_data.extend(header)
		for j in range(1, blocks):
			self.output_data.extend(self._write_block(j, UPDATE_WRITE))
		self.output_data.extend(self._write_block(0, UPDATE_FINISH))
		if self.product.family_id == LPRGB_FAMILY_ID:
			footer = self._create_sysex_start(UPDATE_FOOTER)
			footer.append(0x00)
			footer.extend(RGB_FIRMWARE_FOOTER)
			footer.extend([0x00] * 8)
			footer.append(SYSEX_END)
			self.output_data.extend(footer)
		with open(self.output_file, "wb") as f:
			f.write(self.output_data)
		print(f"Success! Saved to {self.output_file}")

def crc32_bitwise(data: bytes) -> int:
	crc = 0xFFFFFFFF
	for b in data:
		crc ^= (b << 24) & 0xFFFFFFFF
		for _ in range(8):
			if crc & 0x80000000:
				crc = ((crc << 1) ^ 0x04C11DB7) & 0xFFFFFFFF
			else:
				crc = (crc << 1) & 0xFFFFFFFF
	return crc

def uint_from_nibbles(nibs: List[int]) -> int:
	v = 0
	for n in nibs:
		v = ((v << 4) | (n & 0xF)) & 0xFFFFFFFF
	return v

@dataclass
class InitInfo:
	family_id: int
	product_id: int
	version_nibbles: List[int]

@dataclass
class LpxHeaderInfo:
	is_promk3: bool
	version_nibbles: List[int]
	total_size: int
	crc32: int

@dataclass
class SysexMsg:
	msg_type: int
	payload: bytes

def parse_sysex_stream(data: bytes) -> List[SysexMsg]:
	msgs: List[SysexMsg] = []
	i = 0
	n = len(data)
	while i < n:
		while i < n and data[i] != SYSEX_START:
			i += 1
		if i >= n:
			break
		start = i
		j = start + 1
		while j < n and data[j] != SYSEX_END:
			j += 1
		if j >= n:
			raise ValueError("Unterminated SysEx message (no 0xF7).")
		segment = data[start+1:j]
		i = j + 1
		if len(segment) < 5:
			raise ValueError("SysEx message too short for Novation header.")
		if list(segment[:4]) != NOVATION_HEADER:
			continue
		msg_type = segment[4]
		payload = segment[5:]
		msgs.append(SysexMsg(msg_type=msg_type, payload=payload))
	if not msgs:
		raise ValueError("No Novation SysEx messages found.")
	return msgs

def decode_block_7bit_to_32(payload37: bytes, out: bytearray, base: int):
	if len(payload37) != BLOCK_SIZE_7BITS:
		raise ValueError(f"Block payload wrong size: {len(payload37)} (expected {BLOCK_SIZE_7BITS})")
	for i in range(BLOCK_SIZE_BYTES):
		out[base + i] = 0
	for k in range(BLOCK_SIZE_BITS):
		src_idx = k // 7
		bit_in_src = 6 - (k % 7)
		bit = (payload37[src_idx] >> bit_in_src) & 0x1
		dst_byte = base + (k // 8)
		dst_bit  = 7 - (k % 8)
		if bit:
			out[dst_byte] |= (1 << dst_bit)

def syxtobin(input_path: str, output_path: Optional[str]):
	with open(input_path, "rb") as f:
		raw = f.read()
	msgs = parse_sysex_stream(raw)
	init: Optional[InitInfo] = None
	lpx_hdr: Optional[LpxHeaderInfo] = None
	write_blocks: List[bytes] = []
	finish_block: Optional[bytes] = None
	family_id: Optional[int] = None
	product_id: Optional[int] = None
	for m in msgs:
		if m.msg_type == UPDATE_INIT:
			if len(m.payload) < 8:
				raise ValueError("INIT payload too short.")
			family_id = m.payload[0]
			product_id = m.payload[1]
			v0, v1, v2 = m.payload[5] & 0xF, m.payload[6] & 0xF, m.payload[7] & 0xF
			init = InitInfo(family_id=family_id, product_id=product_id, version_nibbles=[v0, v1, v2])
		elif m.msg_type == UPDATE_HEADER:
			if len(m.payload) < 1 + 3 + 3 + 8 + 8:
				raise ValueError("HEADER payload too short.")
			is_promk3 = bool(m.payload[0])
			v0 = m.payload[4] & 0xF
			v1 = m.payload[5] & 0xF
			v2 = m.payload[6] & 0xF
			size_nibs = [x & 0xF for x in m.payload[7:15]]
			crc_nibs  = [x & 0xF for x in m.payload[15:23]]
			total_size = uint_from_nibbles(size_nibs)
			crc_val    = uint_from_nibbles(crc_nibs)
			lpx_hdr = LpxHeaderInfo(is_promk3=is_promk3, version_nibbles=[v0, v1, v2], total_size=total_size, crc32=crc_val)
		elif m.msg_type == UPDATE_WRITE:
			if len(m.payload) != BLOCK_SIZE_7BITS:
				raise ValueError(f"WRITE block has size {len(m.payload)} (expected {BLOCK_SIZE_7BITS})")
			write_blocks.append(m.payload)
		elif m.msg_type == UPDATE_FINISH:
			if len(m.payload) != BLOCK_SIZE_7BITS:
				raise ValueError(f"FINISH block has size {len(m.payload)} (expected {BLOCK_SIZE_7BITS})")
			finish_block = m.payload
	if init is None:
		raise ValueError("No INIT message found — not a Novation updater .syx?")
	if finish_block is None:
		raise ValueError("No FINISH block found — stream incomplete.")
	if family_id is None or product_id is None:
		raise AssertionError("Parser state inconsistent.")
	num_blocks = len(write_blocks) + 1
	total_bytes_by_blocks = num_blocks * BLOCK_SIZE_BYTES
	if lpx_hdr is not None and init.family_id == LPX_FAMILY_ID:
		target_size = lpx_hdr.total_size
	else:
		target_size = total_bytes_by_blocks
	out = bytearray(total_bytes_by_blocks)
	for i, blk in enumerate(write_blocks, start=1):
		decode_block_7bit_to_32(blk, out, i * BLOCK_SIZE_BYTES)
	decode_block_7bit_to_32(finish_block, out, 0)
	out = out[:target_size]
	if output_path is None:
		base, _ = os.path.splitext(input_path)
		output_path = base + ".bin"
	with open(output_path, "wb") as f:
		f.write(out)
	print(f"Success! Saved to {output_path}")

def main():
	p = argparse.ArgumentParser(description="Launchpad firmware SysEx/bin tool")
	g = p.add_mutually_exclusive_group(required=True)
	g.add_argument("--to-syx", action="store_true")
	g.add_argument("--to-bin", action="store_true")
	known, rest = p.parse_known_args()
	if known.to_syx:
		if len(rest) < 3 or len(rest) > 4:
			print("Usage: syxtool.py --to-syx <product> <version> <input> [output]")
			sys.exit(2)
			
		product_flag = rest[0]
		version = rest[1]
		inp = rest[2]
		outp = rest[3] if len(rest) == 4 else None
		
		try:
			product = next(p for p in PRODUCTS if p.flag == product_flag)
		except StopIteration:
			print("Unknown product flag")
			sys.exit(3)
		try:
			BinToSyx(product, version, inp, outp).convert()
		except Exception as e:
			print(f"Error: {e}")
			sys.exit(1)

	elif known.to_bin:
		if len(rest) < 1 or len(rest) > 2:
			print("Usage: syxtool.py --to-bin <input> [output]")
			sys.exit(2)
		inp = rest[0]
		outp = rest[1] if len(rest) == 2 else None
		try:
			syxtobin(inp, outp)
		except Exception as e:
			print(f"Error: {e}")
			sys.exit(1)

if __name__ == "__main__":
	main()
