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

  it("maps transfer-integrity deeplinks to activity types and a date window", () => {
    expect(
      resolveActivityUrlFilters(
        new URLSearchParams("types=TRANSFER_IN,TRANSFER_OUT&from=2026-06-01&to=2026-06-04"),
      ),
    ).toEqual({
      activityTypes: ["TRANSFER_IN", "TRANSFER_OUT"],
      dateFrom: "2026-06-01",
      dateTo: "2026-06-04",
    });
  });

  it("ignores an empty types param", () => {
    expect(resolveActivityUrlFilters(new URLSearchParams("types="))).toEqual({});
  });

  it("clears broker review filter params without dropping unrelated params", () => {
    const cleared = clearActivityUrlFilters(
      new URLSearchParams("tab=investments&account=acct-1&needsReview=true"),
    );

    expect(cleared.toString()).toBe("tab=investments");
  });

  it("clears transfer-integrity deeplink params", () => {
    const cleared = clearActivityUrlFilters(
      new URLSearchParams("tab=investments&types=TRANSFER_IN&from=2026-06-01&to=2026-06-04"),
    );

    expect(cleared.toString()).toBe("tab=investments");
  });
});
