import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createInterface } from "node:readline";
import {
  cpSync,
  mkdtempSync,
  readFileSync,
  writeFileSync,
  rmSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { once } from "node:events";
import type { Page } from "@playwright/test";

/** Test-only IPC transport. All records, paths and queries use the actual Rust engine. */
export class SampleCore {
  readonly child: ChildProcessWithoutNullStreams;
  readonly directory: string;
  readonly sample: string;
  readonly workspaceId: string;
  private pending: { resolve: (v: any) => void; reject: (e: Error) => void }[] =
    [];
  private failure: Error | null = null;

  private constructor(sampleName: string) {
    this.directory = mkdtempSync(resolve(tmpdir(), "ede-gallery-"));
    this.sample = resolve(this.directory, "sample");
    cpSync(resolve("demo", sampleName), this.sample, { recursive: true });
    this.workspaceId = JSON.parse(
      readFileSync(resolve(this.sample, "expected-results.json"), "utf8"),
    ).workspace_id;
    const path = resolve(this.sample, "workspace.yaml");
    writeFileSync(
      path,
      readFileSync(path, "utf8").replace(
        "stable_file_check_ms: 1500",
        "stable_file_check_ms: 10",
      ),
    );
    const executable =
      process.env.EDE_CORE_BIN ||
      resolve(
        `src-tauri/target/debug/excel-data-explorer${process.platform === "win32" ? ".exe" : ""}`,
      );
    this.child = spawn(executable, [
      "rpc",
      resolve(this.directory, "cache.sqlite"),
    ]);
    this.child.stderr.on("data", (b) => process.stderr.write(b));
    createInterface({ input: this.child.stdout }).on("line", (line) => {
      const next = this.pending.shift();
      if (!next) return;
      try {
        const data = JSON.parse(line);
        if ("error" in data) next.reject(new Error(data.error));
        else next.resolve(data.ok);
      } catch (error) {
        next.reject(error as Error);
      }
    });
    this.child.on("error", (error) => this.fail(error));
    this.child.on("exit", (code) =>
      this.fail(new Error(`Rust sample process exited ${code}`)),
    );
  }
  private fail(error: Error) {
    this.failure = error;
    for (const p of this.pending) p.reject(error);
    this.pending = [];
  }
  static async start(sampleName: string) {
    const core = new SampleCore(sampleName);
    try {
      await core.call({
        command: "add_workspace_config",
        config_file_path: resolve(core.sample, "workspace.yaml"),
      });
      await core.call({
        command: "sync_now",
        workspace_id: core.workspaceId,
        source_id: null,
      });
      await core.call({
        command: "select_workspace",
        workspace_id: core.workspaceId,
      });
      return core;
    } catch (error) {
      await core.close();
      throw error;
    }
  }
  call(request: unknown): Promise<any> {
    if (this.failure) return Promise.reject(this.failure);
    return new Promise((resolve, reject) => {
      this.pending.push({ resolve, reject });
      this.child.stdin.write(JSON.stringify(request) + "\n");
    });
  }
  async connect(page: Page) {
    await page.exposeFunction(
      "sampleCoreInvoke",
      (command: string, args: { request: unknown }) => {
        if (command === "dispatch") return this.call(args.request);
        if (command === "pick_workspace_config")
          return resolve(this.sample, "workspace.yaml");
        return null;
      },
    );
    await page.addInitScript(() => {
      const w = window as any;
      w.isTauri = true;
      let callback = 0;
      w.__TAURI_INTERNALS__ = {
        invoke: (cmd: string, args: unknown) => w.sampleCoreInvoke(cmd, args),
        transformCallback: () => ++callback,
        unregisterCallback: () => {},
      };
      w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
    });
    await page.goto("/");
  }
  async close() {
    if (this.child.exitCode === null && !this.failure) {
      const ended = once(this.child, "exit");
      this.child.stdin.end();
      await ended;
    }
    rmSync(this.directory, { recursive: true, force: true });
  }
}
