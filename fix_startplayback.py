#!/usr/bin/env python3

import re
import glob

def fix_startplayback_in_file(filepath):
    """Fix IoEvent::StartPlayback calls to only use first argument"""
    with open(filepath, 'r') as f:
        content = f.read()
    
    original_content = content
    
    # Pattern: IoEvent::StartPlayback(\n  arg1,\n  arg2,\n  arg3,\n) -> IoEvent::StartPlayback(arg1)
    # This regex handles multiline StartPlayback calls with extra arguments
    pattern = r'(IoEvent::StartPlayback\(\s*)(.*?)(\s*,\s*.*?)(\s*,\s*.*?)(\s*\))'
    
    def replace_startplayback(match):
        prefix = match.group(1)  # "IoEvent::StartPlayback("
        first_arg = match.group(2).strip()  # First argument
        suffix = ")"  # Just close the parentheses
        return f"{prefix}{first_arg}{suffix}"
    
    content = re.sub(pattern, replace_startplayback, content, flags=re.DOTALL)
    
    # Write back if changed
    if content != original_content:
        with open(filepath, 'w') as f:
            f.write(content)
        print(f"Fixed StartPlayback calls in {filepath}")
        return True
    return False

def main():
    # Find all Rust files in src/
    rust_files = glob.glob('src/**/*.rs', recursive=True)
    
    fixed_count = 0
    for filepath in rust_files:
        if fix_startplayback_in_file(filepath):
            fixed_count += 1
    
    print(f"Fixed StartPlayback calls in {fixed_count} files")

if __name__ == "__main__":
    main()