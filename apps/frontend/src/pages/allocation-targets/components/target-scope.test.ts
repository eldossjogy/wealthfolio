import { describe, expect, it } from "vitest";

import { accountScopeFromTarget, accountScopeKey, filterTargetsByScope } from "./target-scope";

describe("allocation target scope helpers", () => {
  const targets = [
    { id: "all", scopeType: "all", scopeId: null },
    { id: "account-a", scopeType: "account", scopeId: "account-a" },
    { id: "account-b", scopeType: "account", scopeId: "account-b" },
    { id: "portfolio-a", scopeType: "portfolio", scopeId: "portfolio-a" },
  ] as const;

  it("matches account and portfolio targets by exact saved scope", () => {
    expect(
      filterTargetsByScope(targets, { type: "account", accountId: "account-a" }).map(
        (target) => target.id,
      ),
    ).toEqual(["account-a"]);

    expect(
      filterTargetsByScope(targets, { type: "portfolio", portfolioId: "portfolio-a" }).map(
        (target) => target.id,
      ),
    ).toEqual(["portfolio-a"]);
  });

  it("uses all-account targets for multi-account filters", () => {
    expect(
      filterTargetsByScope(targets, {
        type: "accounts",
        accountIds: ["account-a", "account-b"],
      }).map((target) => target.id),
    ).toEqual(["all"]);
  });

  it("keeps multi-account scope keys stable regardless of input order", () => {
    expect(accountScopeKey({ type: "accounts", accountIds: ["b", "a"] })).toBe(
      accountScopeKey({ type: "accounts", accountIds: ["a", "b"] }),
    );
  });

  it("converts saved target scope back to an account filter shape", () => {
    expect(accountScopeFromTarget({ scopeType: "all", scopeId: null })).toEqual({ type: "all" });
    expect(accountScopeFromTarget({ scopeType: "account", scopeId: "account-a" })).toEqual({
      type: "account",
      accountId: "account-a",
    });
    expect(accountScopeFromTarget({ scopeType: "portfolio", scopeId: "portfolio-a" })).toEqual({
      type: "portfolio",
      portfolioId: "portfolio-a",
    });
  });
});
