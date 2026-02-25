// Writes minimal 16x16 icon.ico (24bpp DIB) so Windows RC accepts it. Run: node gen-icon.js
const fs = require('fs');
const path = require('path');
const dir = path.join(__dirname, 'icons');
if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });

// ICO: header 6 + dir entry 16 + BMP (40-byte header + pixels)
// 24bpp, 16x16: row = 16*3 = 48, stride = 52 (pad to 4), 16 rows = 832 bytes
const bmpSize = 40 + 832;
const icoEntryOffset = 6 + 16;

const icoHeader = Buffer.alloc(22);
icoHeader.writeUInt16LE(0, 0);
icoHeader.writeUInt16LE(1, 2);
icoHeader.writeUInt16LE(1, 4);
icoHeader.writeUInt8(16, 6);   // width
icoHeader.writeUInt8(16, 7);   // height
icoHeader.writeUInt8(0, 8);    // palette
icoHeader.writeUInt8(0, 9);
icoHeader.writeUInt16LE(1, 10);  // planes
icoHeader.writeUInt16LE(24, 12);  // bpp
icoHeader.writeUInt32LE(bmpSize, 14);
icoHeader.writeUInt32LE(icoEntryOffset, 18);

const bmp = Buffer.alloc(40 + 832);
bmp.writeUInt32LE(40, 0);      // BITMAPINFOHEADER.biSize
bmp.writeUInt32LE(16, 4);      // width
bmp.writeUInt32LE(32, 8);      // height (16*2 for ICO)
bmp.writeUInt16LE(1, 12);
bmp.writeUInt16LE(24, 14);
bmp.writeUInt32LE(0, 16);     // BI_RGB
bmp.writeUInt32LE(0, 20);
bmp.writeUInt32LE(0, 24);
bmp.writeUInt32LE(0, 28);
bmp.writeUInt32LE(0, 32);
bmp.writeUInt32LE(0, 36);
// Pixels: bottom-up, BGR, 16 rows of 52 bytes (48 + 4 pad)
for (let row = 15; row >= 0; row--) {
  const base = 40 + row * 52;
  for (let col = 0; col < 16; col++) {
    bmp[base + col * 3] = 255;     // B
    bmp[base + col * 3 + 1] = 0;  // G
    bmp[base + col * 3 + 2] = 0;   // R
  }
}

const ico = Buffer.concat([icoHeader, bmp]);
fs.writeFileSync(path.join(dir, 'icon.ico'), ico);
console.log('icons/icon.ico written (24bpp DIB)');
