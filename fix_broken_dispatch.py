#!/usr/bin/env python3

import re

# Read the file
with open('src/app.rs', 'r') as f:
    content = f.read()

# Fix broken dispatch statements
# Pattern 1: self.dispatch(// IoEvent::Something(args));  
# This should become: // self.dispatch(IoEvent::Something(args));
content = re.sub(r'self\.dispatch\(// (IoEvent::[^;]+);', r'// self.dispatch(\1);', content)

# Pattern 2: multi-line dispatch statements that got broken
# self.dispatch(// IoEvent::Something(
#   args...
# ));
content = re.sub(r'self\.dispatch\(// (IoEvent::[^(]+\([^)]*\))', r'// self.dispatch(\1)', content)

# Fix incomplete multiline dispatch statements that span multiple lines
lines = content.split('\n')
new_lines = []
in_broken_dispatch = False
dispatch_buffer = []

for line in lines:
    if 'self.dispatch(//' in line and not line.strip().endswith(');'):
        # Start of a broken multiline dispatch
        in_broken_dispatch = True
        dispatch_buffer = ['    // ' + line.strip().replace('self.dispatch(//', 'self.dispatch(')]
    elif in_broken_dispatch:
        if line.strip().endswith('));'):
            # End of broken dispatch
            dispatch_buffer.append('    // ' + line.strip())
            # Add all commented lines
            new_lines.extend(dispatch_buffer)
            in_broken_dispatch = False
            dispatch_buffer = []
        else:
            # Middle of broken dispatch
            dispatch_buffer.append('    // ' + line.strip())
    else:
        new_lines.append(line)

content = '\n'.join(new_lines)

# Write back
with open('src/app.rs', 'w') as f:
    f.write(content)

print("Fixed broken dispatch statements")