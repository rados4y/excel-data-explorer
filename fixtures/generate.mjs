import { spawnSync } from "node:child_process";
const result = spawnSync(
  process.platform === "win32" ? "python" : "python3",
  ["fixtures/generate_demo_data.py", ...process.argv.slice(2)],
  { stdio: "inherit" },
);
if (result.error) {
  console.error(
    "Python 3 is required to regenerate demo workbooks:",
    result.error.message,
  );
  process.exit(1);
}
process.exit(result.status ?? 1);
