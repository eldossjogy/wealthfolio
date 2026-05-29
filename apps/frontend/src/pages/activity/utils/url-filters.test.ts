import { describe, expect, it } from "vitest";
import { clearActivityUrlFilters, resolveActivityUrlFilters } from "./url-filters";

describe("resolveActivityUrlFilters", () => {
  it("maps review links to an account pending-review filter", () => {
    expect(
      resolveActivityUrlFilters(new URLSearchParams("account=acct-1&needsReview=true")),
    ).toEqual({
      accountScope: { type: "account", accountId: "acct-1" },
      statusFilter: "pending",
    });
  });

  it("ignores unrelated or false review params", () => {
    expect(resolveActivityUrlFilters(new URLSearchParams("needsReview=false"))).toEqual({});
    expect(resolveActivityUrlFilters(new URLSearchParams("tab=spending"))).toEqual({});
  });

  it("clears broker review filter params without dropping unrelated params", () => {
    const cleared = clearActivityUrlFilters(
      new URLSearchParams("tab=investments&account=acct-1&needsReview=true"),
    );

    expect(cleared.toString()).toBe("tab=investments");
  });
});
