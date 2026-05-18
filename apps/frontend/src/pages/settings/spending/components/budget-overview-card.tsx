import { Link } from "react-router-dom";

import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  Icons,
} from "@wealthfolio/ui";

import { useBudget } from "@/features/spending/hooks/use-budget";
import { formatAmount } from "@/lib/utils";

const MAX_VISIBLE = 6;

export function BudgetOverviewCard() {
  const { data: budget, isLoading: budgetLoading } = useBudget();
  const isLoading = budgetLoading;
  const spendingTarget = budget?.computed.totals.spendingPlanned ?? 0;
  const incomeTarget = budget?.computed.totals.incomePlanned ?? 0;
  const currency = budget?.computed.currency ?? "USD";
  const budgetRows = budget?.computed.groupRows.flatMap((group) => group.categories) ?? [];
  const isEmpty = !isLoading && spendingTarget <= 0 && incomeTarget <= 0;

  const visible = budgetRows.filter((row) => row.target > 0).slice(0, MAX_VISIBLE);
  const overflow = Math.max(0, budgetRows.filter((row) => row.target > 0).length - visible.length);

  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between gap-2 space-y-0 p-4 pb-3">
        <div className="min-w-0 space-y-0.5">
          <CardTitle className="text-sm font-medium">Budget setup</CardTitle>
          <CardDescription className="text-xs">
            {isEmpty
              ? "Groups, default monthly targets, and rollover — the baseline used for every month."
              : `Groups, default monthly targets, and rollover · ${visible.length + overflow} target${visible.length + overflow === 1 ? "" : "s"}`}
          </CardDescription>
        </div>
        <Button asChild variant="ghost" size="sm" className="-mt-1 shrink-0">
          <Link to="/settings/spending/setup">
            Manage
            <Icons.ChevronRight className="ml-1 h-3.5 w-3.5" />
          </Link>
        </Button>
      </CardHeader>
      <CardContent className="p-4 pt-0">
        {isLoading ? (
          <div className="bg-muted/40 h-12 w-full animate-pulse rounded-md" />
        ) : isEmpty ? (
          <div className="space-y-3 py-2">
            <div>
              <div className="text-foreground text-sm font-medium">No budget set up yet</div>
              <p className="text-muted-foreground text-xs">
                Define groups and default targets to use as the baseline each month.
              </p>
            </div>
            <Button asChild size="sm">
              <Link to="/settings/spending/setup">
                <Icons.Plus className="mr-1.5 h-3.5 w-3.5" />
                Set up budget
              </Link>
            </Button>
          </div>
        ) : (
          <div className="space-y-3">
            <div className="grid grid-cols-2 gap-3">
              <div className="bg-muted/30 rounded-md px-3 py-2">
                <div className="text-muted-foreground text-[11px]">Monthly spending</div>
                <div className="text-foreground text-sm font-semibold tabular-nums">
                  {formatAmount(spendingTarget, currency)}
                </div>
              </div>
              <div className="bg-muted/30 rounded-md px-3 py-2">
                <div className="text-muted-foreground text-[11px]">Monthly income</div>
                <div className="text-foreground text-sm font-semibold tabular-nums">
                  {formatAmount(incomeTarget, currency)}
                </div>
              </div>
            </div>
            {visible.length > 0 && (
              <div className="flex flex-wrap items-center gap-1.5">
                {visible.map((row) => {
                  return (
                    <span
                      key={row.categoryId}
                      className="bg-muted/60 text-foreground inline-flex max-w-[200px] items-center gap-1.5 rounded-full px-2 py-0.5 text-xs"
                    >
                      <span
                        className="h-2 w-2 shrink-0 rounded-full"
                        style={{ backgroundColor: row.color ?? "var(--muted-foreground)" }}
                      />
                      <span className="truncate">{row.name}</span>
                    </span>
                  );
                })}
                {overflow > 0 && (
                  <span className="text-muted-foreground text-xs">+{overflow} more</span>
                )}
              </div>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
