import sys
import json
import subprocess
import struct
import argparse

class FirmwarePatcher:
    def __init__(self, config_file, elf_file, bin_file):
        self.config_file = config_file
        self.elf_file = elf_file
        self.bin_file = bin_file
        self.config = {}
        self.symbols = {}
        self.data = bytearray()
        print(">> Starting patcher")
        ok = self._load_config()
        if not ok:
            print("!! Could not load configuration")
            return
        ok = self._load_symbols()
        if not ok:
            print("!! Could not load symbols")
            return
        try:
            with open(self.bin_file, "rb") as f:
                self.data = bytearray(f.read())
            print(f">> Input binary loaded ({len(self.data)} bytes)")
        except Exception as e:
            print(f"!! Could not read binary: {e}")

    def _parse_int(self, s):
        try:
            if isinstance(s, int):
                return s
            s = str(s).strip()
            if s.startswith("0x") or s.startswith("0X"):
                return int(s, 16)
            return int(s)
        except:
            return None

    def _load_config(self):
        print(f">> Loading config from {self.config_file}")
        try:
            with open(self.config_file, "r") as f:
                self.config = json.load(f)
        except Exception as e:
            print(f"!! JSON error: {e}")
            return False
        if "firmware" not in self.config or "patches" not in self.config:
            print("!! Invalid config: missing 'firmware' or 'patches'")
            return False
        if "base_address" not in self.config["firmware"]:
            print("!! Invalid config: missing 'firmware.base_address'")
            return False
        print(">> Config loaded")
        return True

    def _load_symbols(self):
        print(f">> Extracting symbols from {self.elf_file}")
        try:
            out = subprocess.check_output(
                ["arm-none-eabi-nm", "-g", "--defined-only", self.elf_file],
                text=True
            )
        except Exception as e:
            print(f"!! nm failed: {e}")
            return False
        self.symbols = {}
        for line in out.splitlines():
            parts = line.strip().split()
            if len(parts) >= 3:
                try:
                    addr = int(parts[0], 16)
                    sym = parts[2]
                    self.symbols[sym] = addr
                except:
                    pass
        print(f">> Found {len(self.symbols)} symbols")
        return True

    def _get_base_address(self):
        return self._parse_int(self.config["firmware"]["base_address"])

    def _encode_thumb_bl(self, call_addr, target_addr):
        try:
            imm = target_addr - (call_addr + 4)
            if imm % 2 != 0:
                return None
            imm25 = imm >> 1
            if not (-(1 << 24) <= imm25 <= (1 << 24) - 1):
                return None
            S = (imm25 >> 24) & 1
            I1 = (imm25 >> 23) & 1
            I2 = (imm25 >> 22) & 1
            imm10 = (imm25 >> 11) & 0x3FF
            imm11 = imm25 & 0x7FF
            J1 = (~(I1 ^ S)) & 1
            J2 = (~(I2 ^ S)) & 1
            first = (0b11110 << 11) | (S << 10) | imm10
            second = (0b11 << 14) | (1 << 12) | (J1 << 13) | (J2 << 11) | imm11
            return struct.pack("<HH", first, second)
        except:
            return None

    def _encode_thumb_b(self, call_addr, target_addr):
        try:
            imm = target_addr - (call_addr + 4)
            if imm % 2 != 0:
                return None
            imm11 = imm >> 1
            if not (-(1 << 10) <= imm11 <= (1 << 10) - 1):
                return None
            instr = (0b11100 << 11) | (imm11 & 0x7FF)
            return struct.pack("<H", instr)
        except:
            return None

    def _is_thumb_b_to(self, call_addr, target_addr, data):
        if len(data) < 2:
            return False
        enc = self._encode_thumb_b(call_addr, target_addr)
        if enc is None:
            return False
        return data[0:2] == enc

    def _auto_find_call(self, patch):
        cfg = patch.get("auto_find", {})
        if not cfg.get("enabled", False):
            return None
        original_target = self._parse_int(cfg.get("original_target"))
        if original_target is None:
            print("!! auto_find: invalid original_target")
            return None
        search_range = self._parse_int(cfg.get("search_range", "0x1000"))
        if search_range is None:
            search_range = 0x1000
        base = self._get_base_address()
        found = []
        lim = min(len(self.data) - 4, search_range)
        for off in range(0, lim, 2):
            call_addr = base + off
            enc = self._encode_thumb_bl(call_addr, original_target)
            if enc and self.data[off:off+4] == enc:
                found.append(off)
        if len(found) != 1:
            print(f"!! auto_find: expected 1 call site, found {len(found)} {found}")
            return None
        return found[0]

    def _apply_call_patch(self, patch):
        name = patch.get("name", "call_patch")
        sym = patch.get("target_symbol")
        if not sym or sym not in self.symbols:
            print(f"!! {name}: target symbol missing or unknown")
            return False
        target = self.symbols[sym]
        if patch.get("manual_offset"):
            call_off = self._parse_int(patch["manual_offset"])
        else:
            call_off = self._auto_find_call(patch)
        if call_off is None:
            print(f"!! {name}: no call site found")
            return False
        base = self._get_base_address()
        call_addr = base + call_off
        bl = self._encode_thumb_bl(call_addr, target)
        if not bl:
            print(f"!! {name}: could not encode BL")
            return False
        self.data[call_off:call_off+4] = bl
        print(f"OK {name}: BL @ file+0x{call_off:04X} (addr 0x{call_addr:08X}) -> {sym} 0x{target:08X}")
        return True

    def _apply_raw_patch(self, patch):
        name = patch.get("name", "raw_patch")
        off = self._parse_int(patch.get("offset"))
        data_str = patch.get("data", "")
        if off is None or not data_str:
            print(f"!! {name}: missing offset or data")
            return False
        if data_str.startswith("0x") or data_str.startswith("0X"):
            data_str = data_str[2:]
        try:
            b = bytes.fromhex(data_str)
        except Exception as e:
            print(f"!! {name}: invalid hex: {e}")
            return False
        if off + len(b) > len(self.data):
            print(f"!! {name}: patch data out of bounds")
            return False
        self.data[off:off+len(b)] = b
        print(f"OK {name}: {len(b)} byte(s) @ file+0x{off:04X}")
        return True

    def _apply_nop_patch(self, patch):
        name = patch.get("name", "nop_patch")
        addr = self._parse_int(patch.get("address"))
        count = self._parse_int(patch.get("count", 1)) or 1
        base = self._get_base_address()
        if addr is None or addr < base:
            print(f"!! {name}: invalid address")
            return False
        off = addr - base
        size = count * 2
        if off + size > len(self.data):
            print(f"!! {name}: range out of bounds")
            return False
        nop = b"\x00\xBF"
        for i in range(count):
            o = off + i * 2
            self.data[o:o+2] = nop
        print(f"OK {name}: {count} NOP(s) @ addr 0x{addr:08X} (file+0x{off:04X})")
        return True

    def _apply_replace_bytes_patch(self, patch):
        name = patch.get("name", "replace_bytes_patch")
        addrs = patch.get("addresses", [])
        repl_hex = patch.get("replacement", "0046")
        try:
            repl = bytes.fromhex(repl_hex)
        except:
            print(f"!! {name}: invalid replacement")
            return False
        if not addrs:
            print(f"!! {name}: no addresses")
            return False
        base = self._get_base_address()
        n = 0
        for s in addrs:
            a = self._parse_int(s)
            if a is None or a < base:
                print(f"!! {name}: skipping address {s}")
                continue
            off = a - base
            if off + len(repl) > len(self.data):
                print(f"!! {name}: address 0x{a:08X} out of bounds, skip")
                continue
            self.data[off:off+len(repl)] = repl
            n += 1
        print(f"OK {name}: replaced {n} location(s) with 0x{repl_hex}")
        return n > 0

    def _find_all_calls_to_function(self, target_addr, search_range=None):
        base = self._get_base_address()
        found = []
        if search_range is None:
            search_range = len(self.data)
        limit = min(search_range, len(self.data) - 4)
        for off in range(0, limit, 2):
            addr = base + off
            enc = self._encode_thumb_bl(addr, target_addr)
            if enc and self.data[off:off+4] == enc:
                found.append(off)
        return found

    def _find_all_references_to_function(self, target_addr, search_range=None):
        base = self._get_base_address()
        refs = []
        if search_range is None:
            search_range = len(self.data)
        limit = min(search_range, len(self.data) - 4)
        for off in range(0, limit, 2):
            addr = base + off
            enc_bl = self._encode_thumb_bl(addr, target_addr)
            if enc_bl and self.data[off:off+4] == enc_bl:
                refs.append(off)
                continue
            if self._is_thumb_b_to(addr, target_addr, self.data[off:off+2]):
                refs.append(off)
        tb = struct.pack("<I", target_addr)
        for off in range(0, limit - 4, 1):
            if self.data[off:off+4] == tb:
                refs.append(off)
        return refs

    def _apply_redirect_all_calls_patch(self, patch):
        name = patch.get("name", "redirect_all_calls")
        orig = self._parse_int(patch.get("original_function"))
        sym = patch.get("target_symbol")
        if orig is None or not sym or sym not in self.symbols:
            print(f"!! {name}: invalid parameters")
            return False
        tgt = self.symbols[sym]
        rng = patch.get("search_range")
        sr = self._parse_int(rng) if rng is not None else None
        refs = self._find_all_references_to_function(orig, sr)
        if not refs:
            print(f"!! {name}: no references found")
            return False
        base = self._get_base_address()
        count = 0
        for off in sorted(set(refs)):
            addr = base + off
            is_bl = False
            is_b = False
            enc_bl = self._encode_thumb_bl(addr, orig)
            if enc_bl and self.data[off:off+4] == enc_bl:
                is_bl = True
            if not is_bl and self._is_thumb_b_to(addr, orig, self.data[off:off+2]):
                is_b = True
            if is_bl:
                new_bl = self._encode_thumb_bl(addr, tgt)
                if new_bl:
                    self.data[off:off+4] = new_bl
                    count += 1
                    print(f".  BL @ file+0x{off:04X} -> {sym}")
                else:
                    print(f"!! {name}: cannot encode BL at 0x{addr:08X}")
            elif is_b:
                new_b = self._encode_thumb_b(addr, tgt)
                if new_b:
                    self.data[off:off+2] = new_b
                    count += 1
                    print(f".  B  @ file+0x{off:04X} -> {sym}")
                else:
                    print(f"!! {name}: cannot encode B at 0x{addr:08X}")
            else:
                ta = tgt | 1 if (tgt & 1) == 0 else tgt
                self.data[off:off+4] = struct.pack("<I", ta)
                count += 1
                print(f".  DATA @ file+0x{off:04X} -> 0x{ta:08X}")
        print(f"OK {name}: redirected {count} reference(s) to {sym} 0x{tgt:08X}")
        return count > 0

    def _apply_insert_call_patch(self, patch):
        name = patch.get("name", "insert_call")
        sym = patch.get("target_symbol")
        addr = self._parse_int(patch.get("address"))
        if not sym or sym not in self.symbols or addr is None:
            print(f"!! {name}: invalid parameters")
            return False
        tgt = self.symbols[sym]
        base = self._get_base_address()
        if addr < base:
            print(f"!! {name}: address below base")
            return False
        off = addr - base
        if off + 4 > len(self.data):
            print(f"!! {name}: not enough space")
            return False
        enc = self._encode_thumb_bl(addr, tgt)
        if not enc:
            print(f"!! {name}: cannot encode BL")
            return False
        self.data[off:off+4] = enc
        print(f"OK {name}: BL @ 0x{addr:08X} (file+0x{off:04X}) -> {sym} 0x{tgt:08X}")
        return True

    def apply_patches(self):
        if not self.config or not self.data:
            print("!! Nothing to do (missing config or data)")
            return
        patches = self.config.get("patches", [])
        print(f">> Base: {self.config['firmware']['base_address']}")
        print(f">> Found {len(patches)} patch(es)")
        ok_count = 0
        for p in patches:
            if not p.get("enabled", True):
                print(f"⏸ {p.get('name','patch')}: disabled")
                continue
            t = p.get("type", "unknown")
            print(f">> Applying patch: {p.get('name','patch')} [{t}]")
            changed = False
            if t == "call_patch":
                changed = self._apply_call_patch(p)
            elif t == "raw_patch":
                changed = self._apply_raw_patch(p)
            elif t == "nop_patch":
                changed = self._apply_nop_patch(p)
            elif t == "replace_bytes_patch":
                changed = self._apply_replace_bytes_patch(p)
            elif t == "redirect_all_calls":
                changed = self._apply_redirect_all_calls_patch(p)
            elif t == "insert_call":
                changed = self._apply_insert_call_patch(p)
            else:
                print("!! Unknown patch type")
            if changed:
                ok_count += 1
        print(f">> Done: {ok_count}/{len(patches)} patch(es) succeeded")

    def save(self, output_file):
        try:
            with open(output_file, "wb") as f:
                f.write(self.data)
            print(f">> Saved to {output_file}")
        except Exception as e:
            print(f"!! Could not save file: {e}")

def main():
    parser = argparse.ArgumentParser(description="Simple firmware patcher")
    parser.add_argument("config", help="Config JSON")
    parser.add_argument("elf", help="ELF with symbols")
    parser.add_argument("input", help="Input binary")
    parser.add_argument("output", help="Output file")
    parser.add_argument("--verbose", "-v", action="store_true")
    args = parser.parse_args()
    try:
        p = FirmwarePatcher(args.config, args.elf, args.input)
        p.apply_patches()
        p.save(args.output)
    except Exception as e:
        print(f"!! Runtime error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
