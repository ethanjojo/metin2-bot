// Development asset generation only; requires sharp 0.35.5.
const fs = require('node:fs/promises');
const path = require('node:path');
const sharp = require('sharp');

async function main() {
  const root = path.resolve(__dirname, '../packaging/icons');
  const source = (await fs.readFile(path.join(root, 'logo.svg'), 'utf8'))
    .replace(/[ \t]+$/gm, '').replace(/\r\n/g, '\n');
  if (!source.includes('fill="#ffffff"')) throw new Error('Canonical Companion mark must be white.');
  const sizes = [16, 20, 24, 32, 40, 48, 64, 128, 256];
  const images = new Map();
  for (const size of sizes) {
    // One white mark for the app, shell and setup. A dark edge keeps it
    // readable on a light Windows taskbar without adding a background tile.
    const svg = source.replace(/stroke="none"/g, '')
      .replace(/<path /g, `<path stroke="#181818" stroke-width="${1.2 * 1024 / size}" stroke-linejoin="round" paint-order="stroke fill" `);
    const png = await sharp(Buffer.from(svg)).resize(size, size).png().toBuffer();
    images.set(size, png);
  }
  await fs.writeFile(path.join(root, 'companion-256.png'), images.get(256));
  const directory = Buffer.alloc(6 + 16 * sizes.length);
  directory.writeUInt16LE(1, 2);
  directory.writeUInt16LE(sizes.length, 4);
  let offset = directory.length;
  sizes.forEach((size, i) => {
    const entry = 6 + i * 16;
    const png = images.get(size);
    directory[entry] = directory[entry + 1] = size === 256 ? 0 : size;
    directory.writeUInt16LE(1, entry + 4);
    directory.writeUInt16LE(32, entry + 6);
    directory.writeUInt32LE(png.length, entry + 8);
    directory.writeUInt32LE(offset, entry + 12);
    offset += png.length;
  });
  await fs.writeFile(path.join(root, 'companion.ico'), Buffer.concat([directory, ...sizes.map(size => images.get(size))]));
  // GitHub README light mode needs a dark mark; geometry stays canonical.
  const lightLogo = source.replace(/fill="#ffffff"/g, 'fill="#000000"');
  await fs.writeFile(path.resolve(root, '../../docs/assets/companion-logo-light.svg'), lightLogo);
  console.log(`Generated fixed Companion PNG and ICO with ${sizes.length} resolutions.`);
}
main().catch(error => { console.error(error); process.exitCode = 1; });
