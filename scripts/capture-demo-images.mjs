import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { spawn } from "node:child_process";
const require = createRequire(import.meta.url);
const cli = join(
  dirname(require.resolve("@playwright/test/package.json")),
  "cli.js",
);
const child = spawn(
  process.execPath,
  [cli, "test", "gallery.spec.ts", "spaces.spec.ts"],
  {
    stdio: "inherit",
    env: { ...process.env, EDE_CAPTURE_DEMO_IMAGES: "1" },
  },
);
child.on("error", (error) => {
  console.error(error.message);
  process.exit(1);
});
child.on("exit", (code) => process.exit(code ?? 1));
