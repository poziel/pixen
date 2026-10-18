from pathlib import Path
import struct,zlib
p=Path(__file__).parent
# GIF application extension after the global color table.
f=p/'floating-garden.gif';b=f.read_bytes()
if b'NETSCAPE2.0' not in b:
 n=13+(3*(2**((b[10]&7)+1)) if b[10]&128 else 0)
 b=b[:n]+b'\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00'+b[n:]
f.write_bytes(b)
f=p/'floating-garden.apng';b=bytearray(f.read_bytes());i=b.index(b'acTL');b[i+8:i+12]=struct.pack('>I',0);b[i+12:i+16]=struct.pack('>I',zlib.crc32(b[i:i+12])&0xffffffff);f.write_bytes(b)
f=p/'floating-garden.webp';b=bytearray(f.read_bytes());i=b.index(b'ANIM');b[i+12:i+14]=b'\x00\x00';f.write_bytes(b)
