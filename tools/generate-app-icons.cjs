// Development asset generation only; requires sharp 0.35.5.
const fs = require('node:fs/promises');
const path = require('node:path');
const sharp = require('sharp');

async function main() {
  const root = path.resolve(__dirname, '../packaging/icons');
  const source = await fs.readFile(path.join(root, 'logo.svg'), 'utf8');
  const dark = source.replace(/fill="#000000"/g, 'fill="#ffffff"');
  await fs.writeFile(path.join(root, 'logo-dark.svg'), dark);
  for (const [theme, svg] of [['light', source], ['dark', dark]]) {
    await sharp(Buffer.from(svg)).resize(256, 256).png().toFile(path.join(root, `metinbot-${theme}-256.png`));
  }
  const sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256, 512, 1024];
  const images = new Map();
  for (const size of sizes) {
    // Preserve the supplied black mark and transparency. A subpixel light edge
    // keeps the silhouette visible on dark desktop surfaces at every size.
    const svg = source.replace(/stroke="none"/g, '')
      .replace(/<path /g, `<path stroke="#ffffff" stroke-width="${1.2 * 1024 / size}" stroke-linejoin="round" paint-order="stroke fill" `);
    const png = await sharp(Buffer.from(svg)).resize(size, size).png().toBuffer();
    images.set(size, png);
    await fs.writeFile(path.join(root, `metinbot-${size}.png`), png);
  }
  const icoSizes = sizes.filter(size => size <= 256);
  const directory = Buffer.alloc(6 + 16 * icoSizes.length);
  directory.writeUInt16LE(1, 2);
  directory.writeUInt16LE(icoSizes.length, 4);
  let offset = directory.length;
  icoSizes.forEach((size, i) => {
    const entry = 6 + i * 16;
    const png = images.get(size);
    directory[entry] = directory[entry + 1] = size === 256 ? 0 : size;
    directory.writeUInt16LE(1, entry + 4);
    directory.writeUInt16LE(32, entry + 6);
    directory.writeUInt32LE(png.length, entry + 8);
    directory.writeUInt32LE(offset, entry + 12);
    offset += png.length;
  });
  await fs.writeFile(path.join(root, 'metinbot.ico'), Buffer.concat([directory, ...icoSizes.map(size => images.get(size))]));
  const icns = [[16, 'icp4'], [32, 'icp5'], [64, 'icp6'], [128, 'ic07'], [256, 'ic08'], [512, 'ic09'], [1024, 'ic10']].map(([size, type]) => {
    const png = images.get(size);
    const header = Buffer.alloc(8);
    header.write(type, 0, 'ascii');
    header.writeUInt32BE(png.length + 8, 4);
    return Buffer.concat([header, png]);
  });
  const header = Buffer.alloc(8);
  header.write('icns');
  header.writeUInt32BE(8 + icns.reduce((sum, chunk) => sum + chunk.length, 0), 4);
  await fs.writeFile(path.join(root, 'metinbot.icns'), Buffer.concat([header, ...icns]));
  console.log(`Generated ${sizes.length} transparent PNG sizes, ICO and ICNS.`);
}
main().catch(error => { console.error(error); process.exitCode = 1; });
