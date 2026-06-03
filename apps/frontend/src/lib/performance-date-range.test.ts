import { describe, expect, it } from "vitest";
import { getPerformanceDateRangeForRequest, isAllTimeDateRange } from "./performance-date-range";

describe("performance date range helpers", () => {
  it("treats the all-time sentinel as an all-time request", () => {
    const range = { from: new Date(1970, 0, 1), to: new Date(2026, 0, 1) };

    expect(isAllTimeDateRange(range)).toBe(true);
    expect(getPerformanceDateRangeForRequest(range)).toBeUndefined();
  });

  it("handles the ISO all-time sentinel used by interval controls", () => {
    const range = { from: new Date("1970-01-01"), to: new Date(2026, 0, 1) };

    expect(isAllTimeDateRange(range)).toBe(true);
    expect(getPerformanceDateRangeForRequest(range)).toBeUndefined();
  });

  it("uses the interval code as the source of truth when it is available", () => {
    const range = { from: new Date(2026, 0, 1), to: new Date(2026, 1, 1) };

    expect(getPerformanceDateRangeForRequest(range, "ALL")).toBeUndefined();
  });

  it("keeps bounded ranges intact", () => {
    const range = { from: new Date(2026, 0, 1), to: new Date(2026, 1, 1) };

    expect(isAllTimeDateRange(range)).toBe(false);
    expect(getPerformanceDateRangeForRequest(range, "1M")).toBe(range);
  });
});
