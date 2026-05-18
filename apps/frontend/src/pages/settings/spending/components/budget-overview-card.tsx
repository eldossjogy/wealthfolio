import { useMemo } from "react";
import { Link } from "react-router-dom";

import { Button, Icons } from "@wealthfolio/ui";

import { useBudget } from "@/features/spending/hooks/use-budget";
import { cn } from "@/lib/utils";

import { formatAmountWhole } from "./format";

export function BudgetOverviewCard() {
  const { data: budget, isLoading } = useBudget();

  const spendingPlanned = budget?.computed.totals.spendingPlanned ?? 0;
  const incomePlanned = budget?.computed.totals.incomePlanned ?? 0;
  const currency = budget?.computed.currency ?? "USD";

  const groups = useMemo(() => {
    return (budget?.computed.groupRows ?? []).map((row) => ({
      id: row.group.id,
      name: row.group.name,
      color: row.group.color,
      planned: row.plannedTotal,
    }));
  }, [budget?.computed.groupRows]);

  const fundedGroups = groups.filter((g) => g.planned > 0);
  const unfundedGroups = groups.filter((g) => g.planned <= 0);
  const totalPlanned = fundedGroups.reduce((sum, g) => sum + g.planned, 0) || 1;

  const pctOfIncome =
    incomePlanned > 0 ? Math.round((spendingPlanned / incomePlanned) * 100) : null;
  const isOver = pctOfIncome !== null && pctOfIncome > 100;

  const isEmpty = !isLoading && spendingPlanned <= 0 && incomePlanned <= 0;

  if (isLoading) {
    return <div className="bg-muted/40 h-44 w-full animate-pulse rounded-lg" />;
  }

  if (isEmpty) {
    return (
      <div className="bg-card rounded-lg border p-6">
        <div className="space-y-3">
          <div>
            <h3 className="text-base font-semibold">Default monthly plan</h3>
            <p className="text-muted-foreground mt-1 text-xs">
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
      </div>
    );
  }

  return (
    <Link
      to="/settings/spending/setup"
      aria-label="Open budget setup"
      className="bg-card hover:border-foreground/20 group flex items-stretch overflow-hidden rounded-lg border transition-all hover:shadow-md"
    >
      <div className="min-w-0 flex-1 p-6">
        <div className="mb-4">
          <h3 className="text-base font-semibold tracking-tight">Default monthly plan</h3>
          <p className="text-muted-foreground mt-1 text-xs">
            Baseline group targets used every month until you change them.
          </p>
        </div>

        <div className="mb-3 flex items-start justify-between gap-4">
          <div>
            <div className="tabular-nums leading-none">
              <span className="text-foreground text-2xl font-semibold tracking-tight">
                {formatAmountWhole(spendingPlanned, currency)}
              </span>
              {incomePlanned > 0 && (
                <span className="text-muted-foreground ml-1 text-base font-normal">
                  / {formatAmountWhole(incomePlanned, currency)}
                </span>
              )}
            </div>
            <div className="text-muted-foreground mt-1.5 text-[10px] font-medium uppercase tracking-widest">
              Planned spending {incomePlanned > 0 && "/ Monthly income"}
            </div>
          </div>
          {pctOfIncome !== null && (
            <span
              className={cn(
                "inline-flex shrink-0 items-center gap-1.5 rounded-full px-2.5 py-1.5 text-xs font-medium",
                isOver ? "bg-destructive/15 text-destructive" : "bg-success/15 text-success",
              )}
            >
              <Icons.AlertCircle className="h-3 w-3" />
              {pctOfIncome}% of income
            </span>
          )}
        </div>

        {fundedGroups.length > 0 && (
          <div className="mb-3.5 flex h-2 w-full gap-0.5 overflow-hidden rounded">
            {fundedGroups.map((g) => (
              <span
                key={g.id}
                className="block h-full"
                style={{
                  width: `${(g.planned / totalPlanned) * 100}%`,
                  background: g.color ?? "var(--muted-foreground)",
                }}
              />
            ))}
          </div>
        )}

        <div className="flex flex-wrap gap-x-3.5 gap-y-1 text-xs">
          {fundedGroups.map((g) => (
            <span key={g.id} className="inline-flex items-center gap-1.5 py-0.5">
              <span
                className="h-2 w-2 rounded-sm"
                style={{ background: g.color ?? "var(--muted-foreground)" }}
              />
              <span className="text-foreground font-medium">{g.name}</span>
              <span className="text-muted-foreground tabular-nums">
                {formatAmountWhole(g.planned, currency)}
              </span>
            </span>
          ))}
          {unfundedGroups.map((g) => (
            <span
              key={g.id}
              className="text-muted-foreground inline-flex items-center gap-1.5 py-0.5"
            >
              <span
                className="h-2 w-2 rounded-sm opacity-40"
                style={{ background: g.color ?? "var(--muted-foreground)" }}
              />
              <span>{g.name}</span>
              <span>—</span>
            </span>
          ))}
        </div>
      </div>

      {/* CTA strip */}
      <div className="bg-muted/30 group-hover:bg-foreground group-hover:text-background text-muted-foreground flex w-24 shrink-0 flex-col items-center justify-center gap-1.5 border-l text-xs font-medium uppercase tracking-widest transition-colors">
        <span>Open plan</span>
        <Icons.ChevronRight className="h-3.5 w-3.5 transition-transform group-hover:translate-x-0.5" />
      </div>
    </Link>
  );
}
