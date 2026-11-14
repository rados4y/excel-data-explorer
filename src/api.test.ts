import { expect, test } from "vitest";
import { freshness, label } from "./api";
import type { SyncStatus } from "./types";
test("never calls stale cache up to date", () => {
  const base: SyncStatus = {
    app_state: "ReadyFromCache",
    paused: false,
    sources: [],
  };
  expect(
    freshness({
      ...base,
      sources: [
        { status: "unavailable", last_success_at: "2026-01-01" } as never,
      ],
    }),
  ).toBe("Cached — source unavailable");
  expect(freshness({ ...base, sources: [{ status: "error" } as never] })).toBe(
    "Error — using cached data",
  );
  expect(freshness({ ...base, paused: true })).toBe("Paused");
});
test("field labels", () => expect(label("created_at")).toBe("Created At"));
