import { describe, expect, it } from "vitest";

import type { DriftRow } from "@/lib/types";
import { hasVisibleAllocation, isOutOfBand } from "./drift-row-utils";

type VisibleAllocationRow = Pick<
  DriftRow,
  "currentBps" | "targetBps" | "currentValue" | "targetValue"
>;
type OutOfBandRow = Pick<DriftRow, "status" | "isRequired">;

describe("hasVisibleAllocation", () => {
  it("keeps tiny non-targeted values even when rounded bps is zero", () => {
    const row: VisibleAllocationRow = {
      currentBps: 0,
      targetBps: 0,
      currentValue: 1,
      targetValue: 0,
    };

    expect(hasVisibleAllocation(row)).toBe(true);
  });

  it("hides exact zero rows", () => {
    const row: VisibleAllocationRow = {
      currentBps: 0,
      targetBps: 0,
      currentValue: 0,
      targetValue: 0,
    };

    expect(hasVisibleAllocation(row)).toBe(false);
  });

  it("keeps target-only rows", () => {
    const row: VisibleAllocationRow = {
      currentBps: 0,
      targetBps: 2500,
      currentValue: 0,
      targetValue: 25_000,
    };

    expect(hasVisibleAllocation(row)).toBe(true);
  });
});

describe("isOutOfBand", () => {
  it("counts non-targeted rows as gaps", () => {
    const row: OutOfBandRow = { status: "not_targeted", isRequired: false };

    expect(isOutOfBand(row)).toBe(true);
  });

  it("counts required underweight and overweight rows as gaps", () => {
    expect(isOutOfBand({ status: "underweight", isRequired: true })).toBe(true);
    expect(isOutOfBand({ status: "overweight", isRequired: true })).toBe(true);
  });

  it("does not count optional underweight or overweight rows as gaps", () => {
    expect(isOutOfBand({ status: "underweight", isRequired: false })).toBe(false);
    expect(isOutOfBand({ status: "overweight", isRequired: false })).toBe(false);
  });
});
