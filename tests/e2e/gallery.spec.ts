import { test, expect, type Page } from "@playwright/test";
import { copyFileSync } from "node:fs";
import { resolve } from "node:path";
import { SampleCore } from "./helpers/core";

test.use({ viewport: { width: 1536, height: 900 } });
async function picture(page: Page, sample: string) {
  if (process.env.EDE_CAPTURE_DEMO_IMAGES === "1") {
    await page.evaluate(() => document.fonts.ready);
    await page.screenshot({
      path: resolve("demo", sample, "preview.png"),
      fullPage: true,
      animations: "disabled",
    });
  }
}

test("sample 01: offers link to invoices, protocols and PDFs", async ({
  page,
}) => {
  const name = "01-offers-and-finance";
  const core = await SampleCore.start(name);
  try {
    await core.connect(page);
    await expect(page.locator(".table-caption")).toContainText("6 results");
    await page
      .getByRole("gridcell", { name: "OFF-001", exact: true })
      .dblclick();
    await page.getByRole("tab", { name: "Invoices", exact: true }).click();
    await expect(page.getByText("INV-1001", { exact: true })).toBeVisible();
    await expect(page.getByText("INV-1045", { exact: true })).toBeVisible();
    await picture(page, name);
    await page.getByRole("tab", { name: "Protocols", exact: true }).click();
    await expect(page.getByText("PR-022", { exact: true })).toBeVisible();
    await page.getByRole("tab", { name: "Documents", exact: true }).click();
    await expect(page.locator(".document-card")).toHaveCount(3);
  } finally {
    await core.close();
  }
});

test("sample 02: alternate headers preserve leading zeros and typed values", async ({
  page,
}) => {
  const name = "02-alternate-layout";
  const core = await SampleCore.start(name);
  try {
    await core.connect(page);
    await expect(page.locator(".table-caption")).toContainText("3 results");
    await page.getByRole("gridcell", { name: "0007", exact: true }).dblclick();
    await expect(
      page.getByRole("heading", { name: "0007", exact: true }),
    ).toBeVisible();
    await expect(page.locator(".fields")).toContainText("123.45");
    await expect(page.locator(".fields")).toContainText("true");
    await expect(page.locator(".fields")).toContainText("2026-04-01");
    await picture(page, name);
  } finally {
    await core.close();
  }
});

test("sample 03: duplicate IDs stay in their own project namespace", async ({
  page,
}) => {
  const name = "03-namespaces";
  const core = await SampleCore.start(name);
  try {
    await core.connect(page);
    await expect(page.locator(".table-caption")).toContainText("4 results");
    await page
      .locator(".data-row")
      .filter({ hasText: "OFF-001" })
      .filter({ hasText: "North Campus" })
      .dblclick();
    await page.getByRole("tab", { name: "Invoices", exact: true }).click();
    await expect(page.getByText("INV-N-001", { exact: true })).toBeVisible();
    await expect(page.getByText("INV-S-001", { exact: true })).toHaveCount(0);
    await picture(page, name);
    await page.getByRole("button", { name: "Close details" }).click();
    await page
      .locator(".data-row")
      .filter({ hasText: "OFF-001" })
      .filter({ hasText: "South Depot" })
      .dblclick();
    await page.getByRole("tab", { name: "Invoices", exact: true }).click();
    await expect(page.getByText("INV-S-001", { exact: true })).toBeVisible();
    await expect(page.getByText("INV-N-001", { exact: true })).toHaveCount(0);
  } finally {
    await core.close();
  }
});

test("sample 04: missing and blocked documents remain visible", async ({
  page,
}) => {
  const name = "04-documents-and-orphans";
  const core = await SampleCore.start(name);
  try {
    await core.connect(page);
    await page
      .getByRole("gridcell", { name: "OFF-001", exact: true })
      .dblclick();
    await page.getByRole("tab", { name: "Documents", exact: true }).click();
    await expect(page.locator(".document-card")).toHaveCount(4);
    await expect(
      page.locator(".document-card").filter({ hasText: "invoice-present.pdf" }),
    ).toContainText("Available");
    await expect(
      page.locator(".document-card").filter({ hasText: "invoice-missing.pdf" }),
    ).toContainText("missing");
    await expect(
      page.locator(".document-card").filter({ hasText: "outside.pdf" }),
    ).toContainText("blocked");
    await expect(
      page.locator(".document-card").filter({ hasText: "blocked.exe" }),
    ).toContainText("blocked");
    await picture(page, name);
    await page
      .getByRole("button", { name: "Open blocked.exe", exact: true })
      .click();
    await expect(page.getByRole("alert")).toContainText("extension");
    const status = await core.call({
      command: "get_sync_status",
      workspace_id: core.workspaceId,
    });
    expect(
      status.sources.find((s: any) => s.source_id === "invoices").warnings,
    ).toBe(1);
  } finally {
    await core.close();
  }
});

test("sample 05: failed sync preserves the last successful generation", async ({
  page,
}) => {
  const name = "05-sync-recovery";
  const core = await SampleCore.start(name);
  try {
    await core.connect(page);
    await expect(page.locator(".table-caption")).toContainText("2 results");
    const active = resolve(core.sample, "data/offers.xlsx");
    copyFileSync(resolve(core.sample, "scenarios/updated.xlsx"), active);
    await page.getByRole("button", { name: "Sync now", exact: true }).click();
    await expect(page.locator(".table-caption")).toContainText("3 results");
    copyFileSync(resolve(core.sample, "scenarios/invalid.xlsx"), active);
    await page.getByRole("button", { name: "Sync now", exact: true }).click();
    await expect(
      page.getByRole("button", {
        name: "Error — using cached data",
        exact: false,
      }),
    ).toBeVisible();
    await expect(page.locator(".table-caption")).toContainText("3 results");
    await page
      .getByRole("button", { name: "Sync center", exact: false })
      .click();
    await expect(
      page.getByRole("dialog", { name: "Sync center" }),
    ).toBeVisible();
    await expect(page.getByText("3 cached records")).toBeVisible();
    await expect(page.locator(".inline-error")).toContainText("duplicate");
    await picture(page, name);
    await page.getByRole("button", { name: "Close dialog" }).click();
    copyFileSync(resolve(core.sample, "scenarios/baseline.xlsx"), active);
    await page.getByRole("button", { name: "Sync now", exact: true }).click();
    await expect(page.locator(".table-caption")).toContainText("2 results");
    await expect(
      page.getByRole("button", { name: "Up to date", exact: false }),
    ).toBeVisible();
  } finally {
    await core.close();
  }
});
