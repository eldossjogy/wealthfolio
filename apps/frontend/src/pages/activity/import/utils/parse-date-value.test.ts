import { describe, expect, it } from "vitest";
import { parseDateValue } from "./draft-utils";

/**
 * Regression coverage for issue #984: Questrade exports dates as
 * "YYYY-MM-DD HH:MM:SS AM/PM" (e.g. "2026-05-04 12:00:00 AM"), which previously
 * failed to parse and surfaced as an epoch date (1969-12-31).
 */
describe("parseDateValue — 12-hour AM/PM (issue #984)", () => {
  it("auto-detects the Questrade format without explicit config", () => {
    const iso = parseDateValue("2026-05-04 12:00:00 AM", "auto");
    // 12:00:00 AM == local midnight of 2026-05-04
    expect(iso.startsWith("2026-05-04")).toBe(true);
    expect(new Date(iso).getHours()).toBe(0);
  });

  it("auto-detects PM correctly (noon, not midnight)", () => {
    const iso = parseDateValue("2026-05-04 12:00:00 PM", "auto");
    expect(iso.startsWith("2026-05-04")).toBe(true);
    expect(new Date(iso).getHours()).toBe(12);
  });

  it("distinguishes 1 AM from 1 PM", () => {
    expect(new Date(parseDateValue("2026-05-04 01:30:00 AM", "auto")).getHours()).toBe(1);
    expect(new Date(parseDateValue("2026-05-04 01:30:00 PM", "auto")).getHours()).toBe(13);
  });

  it("respects the explicit AM/PM preset", () => {
    const iso = parseDateValue("2026-05-04 12:00:00 PM", "YYYY-MM-DD hh:mm:ss A");
    expect(iso.startsWith("2026-05-04")).toBe(true);
    expect(new Date(iso).getHours()).toBe(12);
  });

  it("still parses plain date-only values", () => {
    expect(parseDateValue("2026-05-04", "auto").startsWith("2026-05-04")).toBe(true);
  });
});
