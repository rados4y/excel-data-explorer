import { test, expect, type Page } from "@playwright/test";
import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createInterface } from "node:readline";
import {
  mkdtempSync,
  rmSync,
  cpSync,
  readFileSync,
  writeFileSync,
  renameSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
let child: ChildProcessWithoutNullStreams, dir: string;
let waiting: { resolve: (v: unknown) => void; reject: (e: unknown) => void }[] =
  [];
function rpc(request: unknown): Promise<any> {
  return new Promise((resolve, reject) => {
    waiting.push({ resolve, reject });
    child.stdin.write(JSON.stringify(request) + "\n");
  });
}
async function connect(page: Page) {
  await page.exposeFunction(
    "coreInvoke",
    async (command: string, args: { request: unknown }) => {
      if (command === "dispatch") return rpc(args.request);
      if (command === "pick_workspace_config")
        return resolve("workspaces/alternate-workspace.yaml");
      return null;
    },
  );
  await page.addInitScript(() => {
    const w = window as any;
    w.isTauri = true;
    let cb = 0;
    w.__TAURI_INTERNALS__ = {
      invoke: (cmd: string, args: unknown) => w.coreInvoke(cmd, args),
      transformCallback: () => ++cb,
      unregisterCallback: () => {},
    };
    w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
  });
  await page.goto("/");
}
test.beforeAll(async () => {
  dir = mkdtempSync(resolve(tmpdir(), "ede-e2e-"));
  cpSync(resolve("fixtures/demo"), resolve(dir, "fixtures"), {
    recursive: true,
  });
  const yaml = readFileSync(resolve("workspaces/demo-workspace.yaml"), "utf8")
    .replace(
      "../fixtures/demo/data",
      resolve(dir, "fixtures/data").replaceAll("\\", "/"),
    )
    .replace(
      "../fixtures/demo/documents",
      resolve(dir, "fixtures/documents").replaceAll("\\", "/"),
    )
    .replace("stable_file_check_ms: 1500", "stable_file_check_ms: 10");
  writeFileSync(resolve(dir, "workspace.yaml"), yaml);
  const executable =
    process.env.EDE_CORE_BIN ||
    resolve(
      `src-tauri/target/debug/excel-data-explorer${process.platform === "win32" ? ".exe" : ""}`,
    );
  child = spawn(executable, ["rpc", resolve(dir, "cache.sqlite")]);
  child.stderr.on("data", (b) => process.stderr.write(b));
  createInterface({ input: child.stdout }).on("line", (line) => {
    const item = waiting.shift();
    if (!item) return;
    const data = JSON.parse(line);
    if ("error" in data) item.reject(new Error(data.error));
    else item.resolve(data.ok);
  });
  child.on("exit", (code) => {
    for (const item of waiting)
      item.reject(new Error(`Rust core exited ${code}`));
    waiting = [];
  });
  await rpc({
    command: "add_workspace_config",
    config_file_path: resolve(dir, "workspace.yaml"),
  });
  await rpc({ command: "sync_now", workspace_id: "demo", source_id: null });
});
test.afterAll(async () => {
  child?.stdin.end();
  await new Promise((r) => child.once("exit", r));
  rmSync(dir, { recursive: true, force: true });
});
test("search, virtualized rows, drill into invoices and validate a PDF", async ({
  page,
}) => {
  await connect(page);
  await expect(
    page.getByRole("heading", { name: "Offers", exact: false }),
  ).toBeVisible();
  await expect(
    page.getByRole("gridcell", { name: "OFF-001", exact: true }),
  ).toBeVisible();
  expect(await page.locator(".data-row").count()).toBeLessThan(45);
  await page.getByLabel("Search records").fill("OFF-001");
  await expect(page.locator(".table-caption")).toContainText("1 results");
  await page.getByRole("gridcell", { name: "OFF-001", exact: true }).dblclick();
  await expect(
    page.getByRole("heading", { name: "OFF-001", exact: true }),
  ).toBeVisible();
  await page.getByRole("tab", { name: "Invoices", exact: true }).click();
  await expect(page.getByText("INV-1001", { exact: true })).toBeVisible();
  await expect(page.getByText("INV-1045", { exact: true })).toBeVisible();
  await page
    .getByRole("button")
    .filter({ has: page.getByText("INV-1001", { exact: true }) })
    .dblclick();
  await expect(page.getByRole("heading", { name: "INV-1001" })).toBeVisible();
  await page.getByRole("tab", { name: "Documents", exact: true }).click();
  await expect(
    page.getByText("invoice-1001.pdf", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Open invoice-1001.pdf", exact: true })
    .click();
  await expect(page.getByRole("alert")).toHaveCount(0);
  if (process.env.EDE_CAPTURE_DEMO_IMAGES === "1") {
    await page.screenshot({
      path: "docs/explorer-screenshot.png",
      fullPage: true,
    });
  }
});
test("filters, pagination, workspace mapping and persisted view", async ({
  page,
}) => {
  await connect(page);
  await page.getByLabel("Clear search").click();
  await expect(page.locator(".table-caption")).toContainText("120 results");
  await page.getByLabel("Filter Status").fill("Active");
  await expect(page.locator(".table-caption")).toContainText("40 results");
  await page.waitForTimeout(600);
  await page.reload();
  await expect(page.getByLabel("Filter Status")).toHaveValue("Active");
  await expect(page.locator(".table-caption")).toContainText("40 results");
  await page.getByLabel("Filter Status").clear();
  await expect(page.locator(".table-caption")).toContainText("120 results");
  await page.getByLabel("Next page").click();
  await expect(page.locator(".table-footer")).toContainText("101–120");
  await page
    .getByRole("button", { name: "Add workspace", exact: true })
    .click();
  await expect(
    page.getByRole("heading", { name: "Orders", exact: false }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Sync now", exact: true }).click();
  await expect(
    page.getByRole("gridcell", { name: "0007", exact: true }),
  ).toBeVisible();
});
test("sync center shows source diagnostics and pause preferences", async ({
  page,
}) => {
  await connect(page);
  await page.getByLabel("Workspace", { exact: true }).selectOption("demo");
  await page.getByRole("button", { name: "Sync center", exact: false }).click();
  await expect(page.getByRole("dialog", { name: "Sync center" })).toBeVisible();
  await expect(page.getByText("241 cached records")).toBeVisible();
  await page.getByRole("button", { name: "Pause", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Resume", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Close dialog" }).click();
  await expect(
    page.getByRole("button", { name: "Paused", exact: false }),
  ).toBeVisible();
});

test("offline source keeps browsable records and shows stale status", async ({
  page,
}) => {
  await connect(page);
  await page.getByLabel("Workspace", { exact: true }).selectOption("demo");
  await rpc({ command: "set_sync_paused", paused: false });
  const path = resolve(dir, "fixtures/data/offers.xlsx");
  renameSync(path, path + ".offline");
  try {
    await page.getByRole("button", { name: "Sync now", exact: true }).click();
    await expect(
      page.getByRole("button", {
        name: "Cached — source unavailable",
        exact: false,
      }),
    ).toBeVisible();
    await page.getByLabel("Search records").fill("OFF-001");
    await page
      .getByRole("gridcell", { name: "OFF-001", exact: true })
      .dblclick();
    await expect(
      page.getByRole("heading", { name: "OFF-001", exact: true }),
    ).toBeVisible();
  } finally {
    renameSync(path + ".offline", path);
    await rpc({
      command: "sync_now",
      workspace_id: "demo",
      source_id: "offers",
    });
  }
});
