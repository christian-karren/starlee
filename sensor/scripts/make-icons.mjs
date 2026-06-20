import { mkdir, writeFile } from "node:fs/promises";
import { deflateSync } from "node:zlib";

const sizes = [16, 32, 48, 128];
await mkdir("extension/assets", { recursive: true });
for (const size of sizes) {
  await writeFile(`extension/assets/icon-${size}.png`, png(size));
}

function png(size) {
  const rgba = Buffer.alloc(size * size * 4);
  const center = (size - 1) / 2;
  const outer = size * 0.38;
  const inner = size * 0.17;
  const star = starPoints(center, center, outer, inner);
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      const offset = (y * size + x) * 4;
      const inStar = pointInPolygon(x, y, star);
      const dx = x - center;
      const dy = y - center;
      const inCore = Math.sqrt(dx * dx + dy * dy) < size * 0.11;
      const color = inCore
        ? [255, 138, 91, 255]
        : inStar
          ? [255, 247, 194, 255]
          : [23, 21, 43, 255];
      rgba.set(color, offset);
    }
  }

  const scanlines = Buffer.alloc((size * 4 + 1) * size);
  for (let y = 0; y < size; y += 1) {
    const rowStart = y * (size * 4 + 1);
    scanlines[rowStart] = 0;
    rgba.copy(scanlines, rowStart + 1, y * size * 4, (y + 1) * size * 4);
  }

  const chunks = [
    chunk("IHDR", Buffer.concat([u32(size), u32(size), Buffer.from([8, 6, 0, 0, 0])])),
    chunk("IDAT", deflateSync(scanlines, { level: 9 })),
    chunk("IEND", Buffer.alloc(0))
  ];
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), ...chunks]);
}

function starPoints(cx, cy, outer, inner) {
  return Array.from({ length: 10 }, (_, index) => {
    const radius = index % 2 === 0 ? outer : inner;
    const angle = -Math.PI / 2 + index * Math.PI / 5;
    return [cx + Math.cos(angle) * radius, cy + Math.sin(angle) * radius];
  });
}

function pointInPolygon(x, y, polygon) {
  let inside = false;
  for (let i = 0, j = polygon.length - 1; i < polygon.length; j = i, i += 1) {
    const [xi, yi] = polygon[i];
    const [xj, yj] = polygon[j];
    const intersects = ((yi > y) !== (yj > y)) && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi;
    if (intersects) inside = !inside;
  }
  return inside;
}

function chunk(type, data) {
  const name = Buffer.from(type);
  return Buffer.concat([u32(data.length), name, data, u32(crc32(Buffer.concat([name, data])) >>> 0)]);
}

function u32(value) {
  const buffer = Buffer.alloc(4);
  buffer.writeUInt32BE(value >>> 0);
  return buffer;
}

function crc32(buffer) {
  let crc = -1;
  for (const byte of buffer) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
    }
  }
  return (crc ^ -1) >>> 0;
}
