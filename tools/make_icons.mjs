// Redraws every icon in the app from one shape: three Z's climbing away from a
// sleeper. Run it after changing a colour or the glyph itself.
//
//   npm i -D playwright && node tools/make_icons.mjs
//
// Playwright is not a dependency of the app — icons are committed, and this is
// the recipe for regenerating them, not part of the build.

import { writeFile, mkdir } from "node:fs/promises";
import { chromium } from "playwright";

const OUT_TRAY = "src-tauri/icons";
const OUT_MASTER = "src";

// The family accent. Menu-bar states get their own colours: the icon's colour
// answers "will it sleep", so it cannot also mean "an update is out" — that is
// what the badge is for.
const TEAL = "#39c0b3";
const STATES = {
  calm: "#46c08b",
  blocked: "#f2705c",
  charging: "#7f8bae",
};

/** Three Z's, climbing left to right, drawn as strokes so they scale cleanly.
 *
 * The gaps between them are wider than they look like they need to be: in the
 * menu bar the whole mark is 22 points tall, and strokes that merely come close
 * at 1024 px touch and read as a blob at that size. */
function zzz(color, stroke) {
  return `
    <path d="M11.8 0.9h5.4l-5.4 5.6h5.4" />
    <path d="M5.9 7.1h4.2l-4.2 4.4h4.2" />
    <path d="M1.2 12.1h3.2l-3.2 3.3h3.2" />
  `.replace(
    /<path /g,
    `<path fill="none" stroke="${color}" stroke-width="${stroke}" stroke-linecap="round" stroke-linejoin="round" `,
  );
}

/** The green dot that means "a new version is waiting".
 *
 * Bottom-right, not the usual top-right: the Z's climb towards the top-right,
 * so a badge up there lands on the biggest stroke and both shapes lose. The
 * bottom-right corner is the empty one on this mark. The dark ring stays —
 * it is what separates the dot from whatever is behind it. */
function badge(size) {
  const r = (size * 29) / 128 / 2;
  const edge = size * 0.03;
  const cx = size - r - edge;
  const cy = size - r - edge;
  return `<circle cx="${cx}" cy="${cy}" r="${r}" fill="#2ecc71" stroke="#125a32" stroke-width="${size * 0.023}" />`;
}

function page(size, color, stroke, withBadge) {
  // viewBox 0 0 18 16 is the glyph's own box; the outer svg scales it and
  // leaves the margin the badge needs.
  return `<!doctype html><meta charset="utf-8">
<style>html,body{margin:0;background:transparent}</style>
<svg xmlns="http://www.w3.org/2000/svg" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}">
  <svg x="${size * 0.08}" y="${size * 0.12}" width="${size * 0.84}" height="${size * 0.76}" viewBox="0 0 18 16">
    ${zzz(color, stroke)}
  </svg>
  ${withBadge ? badge(size) : ""}
</svg>`;
}

const jobs = [
  // Master: dock, DMG, README header, profile showcase — one file, several homes.
  { file: `${OUT_MASTER}/nod.png`, size: 1024, color: TEAL, stroke: 1.15, badge: false },
];

for (const [state, color] of Object.entries(STATES)) {
  jobs.push({ file: `${OUT_TRAY}/tray-${state}.png`, size: 44, color, stroke: 1.45, badge: false });
  jobs.push({
    file: `${OUT_TRAY}/tray-${state}-update.png`,
    size: 44,
    color,
    stroke: 1.45,
    badge: true,
  });
}

const browser = await chromium.launch();
const tab = await browser.newPage({ viewport: { width: 1024, height: 1024 } });

for (const job of jobs) {
  await mkdir(job.file.split("/").slice(0, -1).join("/"), { recursive: true });
  await tab.setViewportSize({ width: job.size, height: job.size });
  await tab.setContent(page(job.size, job.color, job.stroke, job.badge));
  const png = await tab.screenshot({ omitBackground: true });
  await writeFile(job.file, png);
  console.log(`${job.file}  ${job.size}px`);
}

await browser.close();
