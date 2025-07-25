#!/usr/bin/env python3

import re
import os
import glob

def fix_uri_in_file(filepath):
    """Fix .uri field access patterns in a Rust file"""
    with open(filepath, 'r') as f:
        content = f.read()
    
    original_content = content
    
    # Pattern 1: track.uri -> format!("spotify:track:{}", track.id.map(...).unwrap_or_default())
    content = re.sub(
        r'(\w+)\.uri\.clone\(\)',
        r'format!("spotify:track:{}", \1.id.as_ref().map(|id| id.to_string()).unwrap_or_default())',
        content
    )
    
    content = re.sub(
        r'(\w+)\.uri',
        r'format!("spotify:track:{}", \1.id.as_ref().map(|id| id.to_string()).unwrap_or_default())',
        content
    )
    
    # Pattern 2: album.uri -> format!("spotify:album:{}", album.id.map(...).unwrap_or_default())
    content = re.sub(
        r'(\w+\.album)\.uri',
        r'format!("spotify:album:{}", \1.id.as_ref().map(|id| id.to_string()).unwrap_or_default())',
        content
    )
    
    # Pattern 3: playlist.uri -> format!("spotify:playlist:{}", playlist.id)
    content = re.sub(
        r'(\w+\.playlist)\.uri', 
        r'format!("spotify:playlist:{}", \1.id)',
        content
    )
    
    # Pattern 4: artist.uri -> format!("spotify:artist:{}", artist.id)
    content = re.sub(
        r'(\w+\.artist)\.uri',
        r'format!("spotify:artist:{}", \1.id)',
        content
    )
    
    # Write back if changed
    if content != original_content:
        with open(filepath, 'w') as f:
            f.write(content)
        print(f"Fixed URI patterns in {filepath}")
        return True
    return False

def main():
    # Find all Rust files in src/
    rust_files = glob.glob('src/**/*.rs', recursive=True)
    
    fixed_count = 0
    for filepath in rust_files:
        if fix_uri_in_file(filepath):
            fixed_count += 1
    
    print(f"Fixed URI patterns in {fixed_count} files")

if __name__ == "__main__":
    main()