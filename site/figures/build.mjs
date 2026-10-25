// Rebuilds the site's isometric figure: `node site/figures/build.mjs`.
// Writes the script and styles to site/static/figures/ and the HTML fragment
// to site/src/figures/, where build.py inlines it with {{figure:<name>}}.
// The output is committed, so building the site needs no Node.
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
process.env.FIG_OUT = join(HERE, "..", "static", "figures");
process.env.FIG_FRAG = join(HERE, "..", "src", "figures");
await import("./ferrite-gate.mjs");
