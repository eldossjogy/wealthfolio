import { useMemo } from "react";
import { Link } from "react-router-dom";

import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
  EmptyPlaceholder,
  Icons,
} from "@wealthfolio/ui";

import { useCategorizationRules } from "@/features/spending/hooks/use-categorization-rules";
import { useTaxonomy } from "@/hooks/use-taxonomies";
import type { TaxonomyCategory } from "@/lib/types";

const MAX_VISIBLE = 4;

export function RulesOverviewCard() {
  const { data: rules = [], isLoading: rulesLoading } = useCategorizationRules();
  const spending = useTaxonomy("spending_categories");
  const income = useTaxonomy("income_sources");
  const isLoading = rulesLoading || spending.isLoading || income.isLoading;

  const categoryMap = useMemo(() => {
    const map = new Map<string, TaxonomyCategory>();
    [...(spending.data?.categories ?? []), ...(income.data?.categories ?? [])].forEach((c) => {
      map.set(c.id, c);
    });
    return map;
  }, [spending.data?.categories, income.data?.categories]);

  const sorted = useMemo(() => [...rules].sort((a, b) => b.priority - a.priority), [rules]);

  const visible = sorted.slice(0, MAX_VISIBLE);
  const overflow = Math.max(0, sorted.length - visible.length);
  const isEmpty = !isLoading && sorted.length === 0;

  return (
    <Card>
      <CardHeader className="flex flex-row items-start justify-between gap-2 space-y-0 p-4 pb-3">
        <div className="min-w-0 space-y-0.5">
          <CardTitle className="text-sm font-medium">Categorization rules</CardTitle>
          {!isEmpty && (
            <CardDescription className="text-xs">
              {`${sorted.length} rule${sorted.length === 1 ? "" : "s"} · auto-tag activities by transaction-name patterns`}
            </CardDescription>
          )}
        </div>
        {!isEmpty && (
          <Button asChild variant="ghost" size="sm" className="-mt-1 shrink-0">
            <Link to="/settings/spending/rules">
              Manage
              <Icons.ChevronRight className="ml-1 h-3.5 w-3.5" />
            </Link>
          </Button>
        )}
      </CardHeader>
      <CardContent className="p-4 pt-0">
        {isLoading ? (
          <div className="space-y-1.5">
            <div className="bg-muted/40 h-7 w-full animate-pulse rounded-md" />
            <div className="bg-muted/40 h-7 w-full animate-pulse rounded-md" />
          </div>
        ) : isEmpty ? (
          <EmptyPlaceholder className="border-0 bg-transparent p-2">
            <EmptyPlaceholder.Title className="mt-0 text-base">No rules yet</EmptyPlaceholder.Title>
            <EmptyPlaceholder.Description className="mb-3">
              Auto-tag transactions by transaction-name patterns.
            </EmptyPlaceholder.Description>
            <Button asChild size="sm">
              <Link to="/settings/spending/rules">Set up rules</Link>
            </Button>
          </EmptyPlaceholder>
        ) : (
          <div className="divide-border/40 -mx-1 divide-y">
            {visible.map((rule) => {
              const cat = rule.categoryId ? categoryMap.get(rule.categoryId) : null;
              return (
                <div key={rule.id} className="flex items-center gap-2 px-1 py-1.5 text-xs">
                  <span className="text-foreground min-w-0 flex-1 truncate font-medium">
                    {rule.name}
                  </span>
                  {cat ? (
                    <span className="bg-muted/60 inline-flex max-w-[160px] items-center gap-1.5 rounded-full px-2 py-0.5">
                      <span
                        className="h-2 w-2 shrink-0 rounded-full"
                        style={{ backgroundColor: cat.color ?? "var(--muted-foreground)" }}
                        aria-hidden="true"
                      />
                      <span className="truncate">{cat.name}</span>
                    </span>
                  ) : rule.activityType ? (
                    <span className="text-muted-foreground/80 bg-muted/60 inline-flex items-center rounded-full px-2 py-0.5">
                      {rule.activityType}
                    </span>
                  ) : (
                    <span className="text-muted-foreground/60 italic">no target</span>
                  )}
                </div>
              );
            })}
            {overflow > 0 && (
              <div className="text-muted-foreground/80 px-1 pt-1.5 text-[11px]">
                +{overflow} more
              </div>
            )}
          </div>
        )}
      </CardContent>
    </Card>
  );
}
