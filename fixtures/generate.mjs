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
if (result.status !== 0) process.exit(result.status ?? 1);
// With no arguments, regenerate both the original fixture and the curated gallery.
if (process.argv.length === 2) {
  const samples = spawnSync(
    process.platform === "win32" ? "python" : "python3",
    ["demo/generate_samples.py"],
    { stdio: "inherit" },
  );
  if (samples.error) {
    console.error(samples.error.message);
    process.exit(1);
  }
  process.exit(samples.status ?? 1);
}
process.exit(0);
