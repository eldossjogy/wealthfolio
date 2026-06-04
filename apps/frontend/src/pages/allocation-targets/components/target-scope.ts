import type { AccountScope, AllocationTarget } from "@/lib/types";

type TargetScope = Pick<AllocationTarget, "scopeType" | "scopeId">;

export function accountScopeKey(scope: AccountScope): string {
  if (scope.type === "all") return "all";
  if (scope.type === "account") return `account:${scope.accountId}`;
  if (scope.type === "portfolio") return `portfolio:${scope.portfolioId}`;
  return `accounts:${[...scope.accountIds].sort().join(",")}`;
}

export function accountScopeFromTarget(target: TargetScope | null): AccountScope | null {
  if (!target) return null;
  if (target.scopeType === "all") return { type: "all" };
  if (target.scopeType === "account" && target.scopeId) {
    return { type: "account", accountId: target.scopeId };
  }
  if (target.scopeType === "portfolio" && target.scopeId) {
    return { type: "portfolio", portfolioId: target.scopeId };
  }
  return null;
}

export function filterTargetsByScope<T extends TargetScope>(
  targets: readonly T[],
  scope: AccountScope,
): T[] {
  if (scope.type === "all") return targets.filter((target) => target.scopeType === "all");
  if (scope.type === "account") {
    return targets.filter(
      (target) => target.scopeType === "account" && target.scopeId === scope.accountId,
    );
  }
  if (scope.type === "portfolio") {
    return targets.filter(
      (target) => target.scopeType === "portfolio" && target.scopeId === scope.portfolioId,
    );
  }
  return targets.filter((target) => target.scopeType === "all");
}
