// Use the optional workspace-local Rust installation in cloud development.
// Normal installations use the user's existing stable Rust toolchain unchanged.
import { existsSync } from "node:fs";
import { resolve, delimiter } from "node:path";
import { spawn } from "node:child_process";
const [tool, ...args] = process.argv.slice(2);
const env = { ...process.env };
const cargo = resolve(".tooling/cargo");
const rustup = resolve(".tooling/rustup");
if (
  existsSync(
    resolve(cargo, "bin", process.platform === "win32" ? "cargo.exe" : "cargo"),
  )
) {
  env.CARGO_HOME = cargo;
  env.RUSTUP_HOME = rustup;
  env.PATH = resolve(cargo, "bin") + delimiter + (env.PATH || "");
}
const child =
  tool === "tauri"
    ? spawn(
        process.execPath,
        [resolve("node_modules/@tauri-apps/cli/tauri.js"), ...args],
        { stdio: "inherit", env },
      )
    : spawn(tool, args, { stdio: "inherit", env });
child.on("error", (error) => {
  console.error(error.message);
  process.exit(1);
});
child.on("exit", (code, signal) => {
  if (signal) process.kill(process.pid, signal);
  else process.exit(code ?? 1);
});
