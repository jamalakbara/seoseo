// MapLibre 6 locates its ESM worker beside its entry module. Turbopack relocates
// that entry into chunks, so serve the matching installed worker and shared module.
import { copyFile, mkdir } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const source = dirname(fileURLToPath(import.meta.resolve("maplibre-gl")));
const destination = fileURLToPath(new URL("../public/maplibre/", import.meta.url));
await mkdir(destination, { recursive: true });
for (const file of ["maplibre-gl-worker.mjs", "maplibre-gl-shared.mjs"]) {
  await copyFile(join(source, file), join(destination, file));
}
