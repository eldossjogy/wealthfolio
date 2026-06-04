import { describe, expect, it } from "vitest";
import { hasReviewableActivityWarnings } from "./import-run-review";

describe("hasReviewableActivityWarnings", () => {
  it("allows activity review whenever imported rows have warnings", () => {
    expect(hasReviewableActivityWarnings(1)).toBe(true);
  });

  it("does not show an activities review link for sync-state-only review", () => {
    expect(hasReviewableActivityWarnings(0)).toBe(false);
    expect(hasReviewableActivityWarnings(undefined)).toBe(false);
  });
});
