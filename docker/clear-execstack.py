"""Clears the executable-stack flag (PF_X in PT_GNU_STACK) of 64-bit
little-endian ELF shared objects. glibc >= 2.41 refuses to dlopen libraries
that request an executable stack.

    python3 clear-execstack.py lib.so [lib.so ...]
"""

import struct
import sys

PT_GNU_STACK = 0x6474E551
PF_X = 1

for path in sys.argv[1:]:
    with open(path, "r+b") as f:
        header = f.read(64)
        if header[:4] != b"\x7fELF" or header[4] != 2 or header[5] != 1:
            sys.exit(f"{path}: not a 64-bit little-endian ELF file")
        (phoff,) = struct.unpack_from("<Q", header, 0x20)
        phentsize, phnum = struct.unpack_from("<HH", header, 0x36)
        for i in range(phnum):
            f.seek(phoff + i * phentsize)
            p_type, p_flags = struct.unpack("<II", f.read(8))
            if p_type == PT_GNU_STACK and p_flags & PF_X:
                f.seek(phoff + i * phentsize + 4)
                f.write(struct.pack("<I", p_flags & ~PF_X))
                print(f"{path}: cleared executable stack")
