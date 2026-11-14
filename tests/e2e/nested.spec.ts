import { test, expect, type Page } from "@playwright/test";
import { copyFileSync, renameSync } from "node:fs";
import { resolve } from "node:path";
import { SampleCore } from "./helpers/core";

test.setTimeout(60000);
test.use({ viewport: { width: 1536, height: 1100 } });
async function prepare() {
  const core = SampleCore.empty();
  const file = resolve(core.sample, "project.xlsx");
  copyFileSync(resolve("demo/06-nested-views/data/project.xlsx"), file);
  const w = await core.call({ command: "add_excel_workbook", file_path: file });
  await core.call({ command: "sync_now", workspace_id: w.id, source_id: null });
  return { core, file, workspace: w.id };
}
async function setup(page: Page) {
  await page
    .getByRole("button", { name: "Related views", exact: true })
    .click();
  for (const [i, title] of ["Invoices", "Protocols"].entries()) {
    await page
      .getByRole("button", { name: "Add related list", exact: true })
      .click();
    await page
      .getByLabel(`Child view ${i + 1}`, { exact: true })
      .selectOption({ label: title });
    await page
      .getByLabel(`Parent matching field ${i + 1}`)
      .selectOption("column_1");
    await page
      .getByLabel(`Child matching field ${i + 1}`)
      .selectOption("column_2");
  }
  if (process.env.EDE_CAPTURE_DEMO_IMAGES === "1")
    await page.screenshot({
      path: "docs/nested-view-editor.png",
      animations: "disabled",
    });
  await page
    .getByRole("button", { name: "Save related views", exact: true })
    .click();
  await expect(page.getByRole("dialog")).toHaveCount(0);
}
test("configure nested worksheet views in-app, select offers, drill down and restore offline", async ({
  page,
}) => {
  const { core, file } = await prepare();
  try {
    await core.connect(page);
    await expect(page.locator(".table-caption")).toContainText("6 results");
    await setup(page);
    await page.getByRole("gridcell", { name: "OFF-001", exact: true }).click();
    const invoices = page.getByRole("region", {
      name: "Related Invoices",
      exact: true,
    });
    const protocols = page.getByRole("region", {
      name: "Related Protocols",
      exact: true,
    });
    await expect(invoices.getByText("INV-1001", { exact: true })).toBeVisible();
    await expect(invoices.getByText("INV-1045", { exact: true })).toBeVisible();
    await expect(protocols.getByText("PR-022", { exact: true })).toBeVisible();
    await expect(invoices.getByText("INV-2001", { exact: true })).toHaveCount(
      0,
    );
    await invoices
      .getByRole("button", {
        name: "Open related Invoices INV-1001",
        exact: true,
      })
      .click();
    await expect(
      page.getByRole("heading", { name: "INV-1001", exact: true }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Back to parent record" }).click();
    await expect(invoices.getByText("INV-1001", { exact: true })).toBeVisible();
    await page.getByRole("gridcell", { name: "OFF-002", exact: true }).click();
    await expect(invoices.getByText("INV-2001", { exact: true })).toBeVisible();
    await expect(invoices.getByText("INV-1001", { exact: true })).toHaveCount(
      0,
    );
    await expect(
      protocols.getByText("No related records.", { exact: true }),
    ).toBeVisible();
    renameSync(file, file + ".offline");
    await page
      .getByRole("button", { name: "Close details", exact: true })
      .click();
    await page.reload();
    await page.getByRole("gridcell", { name: "OFF-001", exact: true }).click();
    await expect(invoices.getByText("INV-1045", { exact: true })).toBeVisible();
    await expect(protocols.getByText("PR-022", { exact: true })).toBeVisible();
    await page
      .getByRole("button", { name: "Related views", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Remove list 2", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Save related views", exact: true })
      .click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await page.getByRole("gridcell", { name: "OFF-001", exact: true }).click();
    await expect(invoices.getByText("INV-1001", { exact: true })).toBeVisible();
    await expect(protocols).toHaveCount(0);
    await expect(page.getByRole("alert")).toHaveCount(0);
  } finally {
    await core.close();
  }
});

test("nested child views retain saved filters and hidden columns and can drill into deeper lists", async ({
  page,
}) => {
  const { core, workspace } = await prepare();
  try {
    const c = await core.call({
      command: "get_workspace_schema",
      workspace_id: workspace,
    });
    const invoices = c.pages.find((p: any) => p.title === "Invoices"),
      protocols = c.pages.find((p: any) => p.title === "Protocols");
    const child = await core.call({
      command: "create_view",
      workspace_id: workspace,
      source_id: invoices.source_id,
      title: "Large invoices",
      columns: ["column_1", "column_4"],
      filters: { column_4: "7500.00" },
      sort: null,
    });
    await core.call({
      command: "save_page",
      workspace_id: workspace,
      page: { ...child, hidden: true },
    });
    await core.call({
      command: "save_nested_views",
      workspace_id: workspace,
      page_id: child.id,
      views: [
        {
          title: "Offer protocols",
          page_id: protocols.id,
          parent_field: "column_2",
          child_field: "column_2",
        },
      ],
    });
    await core.connect(page);
    await page
      .getByRole("button", { name: "Related views", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Add related list", exact: true })
      .click();
    await page
      .getByLabel("Child view 1", { exact: true })
      .selectOption(child.id);
    await page.getByLabel("Parent matching field 1").selectOption("column_1");
    await page.getByLabel("Child matching field 1").selectOption("column_2");
    await page
      .getByRole("button", { name: "Save related views", exact: true })
      .click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await page.getByRole("gridcell", { name: "OFF-001", exact: true }).click();
    const section = page.getByRole("region", {
      name: "Related Large invoices",
      exact: true,
    });
    await expect(section.getByText("INV-1045", { exact: true })).toBeVisible();
    await expect(section.getByText("INV-1001", { exact: true })).toHaveCount(0);
    await expect(
      section.getByRole("columnheader", { name: "OfferID", exact: true }),
    ).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: "Page Large invoices", exact: true }),
    ).toHaveCount(0);
    await section
      .getByRole("button", {
        name: "Open related Large invoices INV-1045",
        exact: true,
      })
      .click();
    await expect(
      page.getByRole("heading", { name: "INV-1045", exact: true }),
    ).toBeVisible();
    await expect(
      page.locator(".fields dt").filter({ hasText: "OfferID" }),
    ).toHaveCount(0);
    await expect(
      page
        .getByRole("region", { name: "Related Offer protocols", exact: true })
        .getByText("PR-022", { exact: true }),
    ).toBeVisible();
    await page.getByRole("button", { name: "Back to parent record" }).click();
    await expect(section.getByText("INV-1045", { exact: true })).toBeVisible();
  } finally {
    await core.close();
  }
});
