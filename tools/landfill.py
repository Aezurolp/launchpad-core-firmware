import json
import sys
import os
from pathlib import Path

def parse_address(addr_str):
    if addr_str.startswith('0x') or addr_str.startswith('0X'):
        return int(addr_str, 16)
    return int(addr_str)

def landfill_binary(config_file):
    try:
        with open(config_file, 'r') as f:
            config = json.load(f)
    except FileNotFoundError:
        print(f"Error: Configuration file '{config_file}' not found")
        return False
    except json.JSONDecodeError as e:
        print(f"Error parsing JSON file: {e}")
        return False
    
    required_fields = ['firmware']
    
    if 'firmware' in config:
        if 'original' not in config['firmware'] or 'base_address' not in config['firmware']:
            print("Error: 'firmware.original' and 'firmware.base_address' must be present in configuration")
            return False
        
        original_file = config['firmware']['original']
        base_address_str = config['firmware']['base_address']
        
        if 'landfill' not in config:
            print("Error: 'landfill' section missing in patches file")
            return False
        
        if not config['landfill'].get('enabled', True):
            print("Info: Landfill is disabled, skipping...")
            return True
        
        landfill_config = config['landfill']
        
    else:
        required_fields = ['original', 'base', 'output']
        for field in required_fields:
            if field not in config:
                print(f"Error: Required field '{field}' missing in configuration")
                return False
        
        original_file = config['original']
        base_address_str = config['base']
        landfill_config = config
    
    if 'adresses' not in landfill_config and 'nop_patches' not in landfill_config and 'nops' not in landfill_config:
        print("Error: One of 'adresses', 'nop_patches' or 'nops' must be present in landfill configuration")
        return False
    
    config_dir = Path(config_file).parent
    original_path = config_dir / original_file
    
    if 'firmware' in config:
        original_name = Path(original_file).stem
        output_file = f"{original_name}-LANDFILL.bin"
        output_path = config_dir / output_file
    else:
        output_path = config_dir / config['output']
    
    try:
        with open(original_path, 'rb') as f:
            binary_data = bytearray(f.read())
    except FileNotFoundError:
        print(f"Error: Original file '{original_path}' not found")
        return False
    
    base_address = parse_address(base_address_str)
    
    print(f"Processing: {config.get('name', config.get('firmware', {}).get('description', 'Unknown'))}")
    print(f"Original: {original_path}")
    print(f"Base address: 0x{base_address:08x}")
    print(f"Binary file size: {len(binary_data)} bytes")
    
    total_filled = 0
    if 'adresses' in landfill_config:
        print(f"Processing {len(landfill_config['adresses'])} landfill regions...")
        for i, addr_range in enumerate(landfill_config['adresses']):
            if 'from' not in addr_range or 'to' not in addr_range:
                print(f"Warning: Address range {i} has invalid format, skipped")
                continue
            
            from_addr = parse_address(addr_range['from'])
            to_addr = parse_address(addr_range['to'])
            
            from_offset = from_addr - base_address
            to_offset = to_addr - base_address
            
            if from_offset < 0:
                print(f"Warning: From address 0x{from_addr:08x} is before base address, skipped")
                continue
            
            if to_offset >= len(binary_data):
                print(f"Warning: To address 0x{to_addr:08x} is outside binary file, skipped")
                continue
            
            if from_offset >= to_offset:
                print(f"Warning: Invalid range 0x{from_addr:08x}-0x{to_addr:08x}, skipped")
                continue
            
            fill_size = to_offset - from_offset + 1
            binary_data[from_offset:to_offset + 1] = [0xFF] * fill_size
            total_filled += fill_size
            
            print(f"  Landfill: 0x{from_addr:08x}-0x{to_addr:08x} ({fill_size} bytes)")
    
    total_nops = 0
    nop_list = landfill_config.get('nop_patches', landfill_config.get('nops', []))
    if nop_list:
        print(f"Processing {len(nop_list)} NOP patches...")
        for i, nop_patch in enumerate(nop_list):
            if 'address' not in nop_patch or 'count' not in nop_patch:
                print(f"Warning: NOP patch {i} has invalid format, skipped")
                continue
            
            if 'enabled' in nop_patch and not nop_patch['enabled']:
                print(f"  NOP patch {nop_patch.get('name', f'#{i}')} is disabled, skipped")
                continue
            
            address = parse_address(nop_patch['address'])
            count = int(nop_patch['count'])
            
            offset = address - base_address
            
            if offset < 0:
                print(f"Warning: NOP address 0x{address:08x} is before base address, skipped")
                continue
            
            if offset + count > len(binary_data):
                print(f"Warning: NOP patch at 0x{address:08x} would extend beyond binary file, skipped")
                continue
            
            nop_pattern = []
            full_nops = count // 2
            remaining_bytes = count % 2
            
            for _ in range(full_nops):
                nop_pattern.extend([0x00, 0xBF])
            
            if remaining_bytes:
                nop_pattern.append(0x00)
            
            for j in range(count):
                binary_data[offset + j] = nop_pattern[j]
            
            total_nops += count
            
            patch_name = nop_patch.get('name', f'patch_{i}')
            description = nop_patch.get('description', '')
            desc_str = f" - {description}" if description else ""
            print(f"  NOP: {patch_name} @ 0x{address:08x} ({count} bytes){desc_str}")
    
    total_modified = total_filled + total_nops
    
    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        
        with open(output_path, 'wb') as f:
            f.write(binary_data)
        
        print(f"Output written: {output_path}")
        print(f"Total filled: {total_filled} bytes")
        print(f"Total NOPed: {total_nops} bytes")
        print(f"Total modified: {total_modified} bytes")
        return True
        
    except Exception as e:
        print(f"Error writing output file: {e}")
        return False

def main():
    if len(sys.argv) != 2:
        print("Usage: python3 scripts/landfill.py <config.json>")
        sys.exit(1)
    
    config_file = sys.argv[1]
    
    if not os.path.exists(config_file):
        print(f"Error: Config file '{config_file}' does not exist.")
        sys.exit(1)
    
    success = landfill_binary(config_file)
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()