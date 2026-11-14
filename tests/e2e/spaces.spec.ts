import { test, expect, type Page } from "@playwright/test";
import { copyFileSync } from "node:fs";
import { resolve } from "node:path";
import { SampleCore } from "./helpers/core";

test.setTimeout(45000);
function copy(core: SampleCore, source: string, name: string) {
  const path = resolve(core.sample, name);
  copyFileSync(resolve(source), path);
  return path;
}
async function add(page: Page) {
  await page
    .getByRole("button", { name: "Add Excel workbook", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Add Excel workbooks" }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Add workbooks", exact: true }),
  ).toBeEnabled({ timeout: 15000 });
}
async function capture(page: Page, name: string) {
  if (process.env.EDE_CAPTURE_DEMO_IMAGES === "1")
    await page.screenshot({
      path: `docs/${name}.png`,
      fullPage: true,
      animations: "disabled",
    });
}

test("fresh startup needs no config; create an empty space and add workbook pages to it", async ({
  page,
}) => {
  const core = SampleCore.empty();
  const file = copy(
    core,
    "demo/01-offers-and-finance/data/finance.xlsx",
    "finance.xlsx",
  );
  try {
    await core.connect(page, [file]);
    await expect(
      page.getByRole("heading", {
        name: "Start with your first Excel workbook",
      }),
    ).toBeVisible();
    await capture(page, "spaces-welcome");
    await page
      .getByRole("button", { name: "Create an empty space", exact: true })
      .click();
    await page.getByLabel("Space name", { exact: true }).fill("Finance area");
    await page
      .getByRole("button", { name: "Create space", exact: true })
      .click();
    await expect(
      page.getByRole("heading", {
        name: "This space is ready for Excel",
        exact: true,
      }),
    ).toBeVisible();
    await page.reload();
    await expect(page.getByLabel("Space", { exact: true })).toContainText(
      "Finance area",
    );
    await add(page);
    await page
      .getByLabel("Where to add the workbook")
      .selectOption({ label: "Existing space: Finance area" });
    await page
      .getByRole("button", { name: "Add workbooks", exact: true })
      .click();
    await expect(
      page.getByRole("button", { name: "Page Invoices", exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Page Protocols", exact: true }),
    ).toBeVisible();
    await expect(page.locator(".table-caption")).toContainText("8 results");
    await page
      .getByRole("button", { name: "Page Protocols", exact: true })
      .click();
    await expect(page.locator(".table-caption")).toContainText("3 results");
    await capture(page, "spaces-worksheets");
    await expect(
      page.getByLabel("Space", { exact: true }).locator("option"),
    ).toHaveCount(1);
  } finally {
    await core.close();
  }
});

test("adding multiple Excel files creates a separate named space for each by default", async ({
  page,
}) => {
  const core = SampleCore.empty();
  const files = [
    copy(core, "demo/01-offers-and-finance/data/finance.xlsx", "finance.xlsx"),
    copy(core, "demo/02-alternate-layout/data/orders.xlsx", "orders.xlsx"),
  ];
  try {
    await core.connect(page, files);
    await add(page);
    await page
      .getByLabel("Space name for finance", { exact: true })
      .fill("Finance workbook");
    await page
      .getByLabel("Space name for orders", { exact: true })
      .fill("Orders workbook");
    await page
      .getByRole("button", { name: "Add workbooks", exact: true })
      .click();
    await expect(
      page.getByRole("dialog", { name: "Add Excel workbooks" }),
    ).toHaveCount(0, { timeout: 20000 });
    await expect(
      page.getByRole("heading", { name: "Sales Export", exact: false }),
    ).toBeVisible();
    await expect(page.locator(".table-caption")).toContainText("3 results");
    await expect(
      page.getByLabel("Space", { exact: true }).locator("option"),
    ).toHaveCount(2);
    await page
      .getByLabel("Space", { exact: true })
      .selectOption({ label: "Finance workbook" });
    await expect(
      page.getByRole("button", { name: "Page Invoices", exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Page Sales Export", exact: true }),
    ).toHaveCount(0);
    await expect(
      page.getByRole("heading", { name: "Invoices", exact: false }),
    ).toBeVisible();
    await page.getByLabel("Manage pages and views", { exact: true }).click();
    await page
      .getByRole("button", { name: "Hide page Protocols", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Close dialog", exact: true })
      .click();
    await expect(
      page.getByRole("button", { name: "Page Protocols", exact: true }),
    ).toHaveCount(0);
    await page.reload();
    await expect(
      page.getByRole("heading", { name: "Invoices", exact: false }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Page Protocols", exact: true }),
    ).toHaveCount(0);
    await page.getByLabel("Manage pages and views", { exact: true }).click();
    await page
      .getByRole("button", { name: "Show page Protocols", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Close dialog", exact: true })
      .click();
    await expect(
      page.getByRole("button", { name: "Page Protocols", exact: true }),
    ).toBeVisible();
  } finally {
    await core.close();
  }
});

test("create a filtered view, hide columns in table and details, and restore it after reload", async ({
  page,
}) => {
  const core = SampleCore.empty();
  const file = copy(
    core,
    "demo/01-offers-and-finance/data/offers.xlsx",
    "offers.xlsx",
  );
  try {
    await core.connect(page, [file]);
    await add(page);
    await page
      .getByRole("button", { name: "Add workbooks", exact: true })
      .click();
    await expect(page.locator(".table-caption")).toContainText("6 results");
    await page.getByRole("button", { name: "New view", exact: true }).click();
    await page.getByLabel("View name", { exact: true }).fill("Active offers");
    await page.getByRole("button", { name: "Add filter", exact: true }).click();
    await page
      .getByLabel("Filter field 1", { exact: true })
      .selectOption("column_5");
    await page.getByLabel("Filter value 1", { exact: true }).fill("Active");
    await page.getByLabel("Show column Description", { exact: true }).uncheck();
    await capture(page, "spaces-view-editor");
    await page
      .getByRole("button", { name: "Create view", exact: true })
      .click();
    await expect(
      page.getByRole("heading", { name: "Active offers", exact: false }),
    ).toBeVisible();
    await expect(page.locator(".table-caption")).toContainText("3 results");
    await expect(
      page.getByRole("columnheader", { name: "Description", exact: false }),
    ).toHaveCount(0);
    await page
      .getByRole("gridcell", { name: "OFF-001", exact: true })
      .dblclick();
    await expect(
      page.locator(".fields dt").filter({ hasText: "Description" }),
    ).toHaveCount(0);
    await page.getByRole("button", { name: "Close details" }).click();
    await page.reload();
    await expect(
      page.getByRole("heading", { name: "Active offers", exact: false }),
    ).toBeVisible();
    await expect(page.locator(".table-caption")).toContainText("3 results");
    await page.getByRole("button", { name: "Manage pages and views" }).click();
    await page
      .getByRole("button", { name: "Hide page Offers", exact: true })
      .click();
    await page
      .getByRole("button", { name: "Close dialog", exact: true })
      .click();
    await expect(
      page.getByRole("button", { name: "Page Offers", exact: true }),
    ).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: "Page Active offers", exact: true }),
    ).toBeVisible();
  } finally {
    await core.close();
  }
});

test("worksheet settings can change labels, field types, headers and the stable key in app", async ({
  page,
}) => {
  const core = SampleCore.empty();
  const file = copy(
    core,
    "demo/01-offers-and-finance/data/offers.xlsx",
    "offers.xlsx",
  );
  try {
    await core.connect(page, [file]);
    await add(page);
    await page
      .getByRole("button", { name: "Add workbooks", exact: true })
      .click();
    await expect(page.locator(".table-caption")).toContainText("6 results");
    await page
      .getByRole("button", { name: "Worksheet settings", exact: true })
      .click();
    await page
      .getByLabel("Unique record ID", { exact: true })
      .selectOption("column_1");
    await page
      .getByLabel("Type for Amount", { exact: true })
      .selectOption("decimal");
    await page
      .getByLabel("Label for Amount", { exact: true })
      .fill("Contract value");
    await page
      .getByRole("button", { name: "Read headers from workbook", exact: true })
      .click();
    await expect(
      page.getByRole("button", { name: "Save mapping and sync", exact: true }),
    ).toBeEnabled();
    await expect(
      page.getByLabel("Type for Amount", { exact: true }),
    ).toHaveValue("decimal");
    await capture(page, "spaces-worksheet-settings");
    await page
      .getByRole("button", { name: "Save mapping and sync", exact: true })
      .click();
    await expect(page.getByRole("dialog")).toHaveCount(0);
    await expect(
      page.getByRole("columnheader", { name: "Contract value", exact: false }),
    ).toBeVisible();
    await expect(page.locator(".table-caption")).toContainText("6 results");
    await page
      .getByRole("gridcell", { name: "OFF-001", exact: true })
      .dblclick();
    await expect(
      page.getByRole("heading", { name: "OFF-001", exact: true }),
    ).toBeVisible();
    await expect(page.locator(".fields")).toContainText("12500");
    await expect(page.getByRole("alert")).toHaveCount(0);
  } finally {
    await core.close();
  }
});
