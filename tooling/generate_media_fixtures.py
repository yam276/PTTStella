"""Generate our own CC0 test images with the Python standard library only."""
from pathlib import Path
import struct
import zlib

ROOT = Path(__file__).resolve().parent.parent / 'tests/fixtures/media'

def png():
    def chunk(kind, payload):
        return struct.pack('>I', len(payload)) + kind + payload + struct.pack('>I', zlib.crc32(kind + payload))
    row = bytes([255, 0, 0]) * 32 + bytes([0, 0, 255]) * 32
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 64, 32, 8, 2, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress((b'\0' + row) * 32)) + chunk(b'IEND', b''))

def gif():
    data = bytearray(b'GIF89a' + struct.pack('<HHBBB', 64, 32, 0x80, 0, 0) + bytes([255, 0, 0, 0, 0, 255]))
    data.extend(b'\x21\xff\x0bNETSCAPE2.0\x03\x01\0\0\0')
    for colour in [0, 1]:
        data.extend(b'\x21\xf9\x04\x04' + struct.pack('<H', 30) + b'\0\0')
        data.extend(b'\x2c' + struct.pack('<HHHHB', 0, 0, 64, 32, 0) + b'\x02')
        # Frequent clears keep every LZW code three bits wide, independently decodable.
        codes = [code for _ in range(64 * 32) for code in (4, colour)] + [5]
        bits = 0
        count = 0
        packed = bytearray()
        for code in codes:
            bits |= code << count
            count += 3
            while count >= 8:
                packed.append(bits & 255)
                bits >>= 8
                count -= 8
        if count:
            packed.append(bits & 255)
        for start in range(0, len(packed), 255):
            block = packed[start:start + 255]
            data.append(len(block))
            data.extend(block)
        data.append(0)
    data.append(0x3b)
    return bytes(data)

if __name__ == '__main__':
    ROOT.mkdir(parents=True, exist_ok=True)
    (ROOT / 'colours.png').write_bytes(png())
    (ROOT / 'motion.gif').write_bytes(gif())
