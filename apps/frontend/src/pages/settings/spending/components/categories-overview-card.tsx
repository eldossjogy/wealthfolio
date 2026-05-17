import { useMemo } from "react";

import { useTaxonomy } from "@/hooks/use-taxonomies";
import type { TaxonomyCategory } from "@/lib/types";

import { OverviewCard, type OverviewChip } from "./overview-card";

interface Props {
  variant: "expense" | "income";
}

const TAXONOMY_ID = {
  expense: "spending_categories",
  income: "income_sources",
} as const;

export function CategoriesOverviewCard({ variant }: Props) {
  const taxonomyId = TAXONOMY_ID[variant];
  const { data, isLoading } = useTaxonomy(taxonomyId);

  const { chips, total, topCount, subCount } = useMemo(() => {
    const cats = (data?.categories ?? []) as TaxonomyCategory[];
    const top = cats.filter((c) => !c.parentId).sort((a, b) => a.sortOrder - b.sortOrder);
    const sub = cats.filter((c) => c.parentId);
    const items: OverviewChip[] = top.map((c) => ({
      id: c.id,
      name: c.name,
      color: c.color,
    }));
    return {
      chips: items,
      total: cats.length,
      topCount: top.length,
      subCount: sub.length,
    };
  }, [data?.categories]);

  const title = variant === "expense" ? "Expense categories" : "Income sources categories";
  const description =
    total === 0
      ? variant === "expense"
        ? "Group transactions into expense buckets."
        : "Group transactions into income sources."
      : variant === "expense"
        ? `${topCount} top-level · ${subCount} subcategories`
        : `${topCount} sources${subCount > 0 ? ` · ${subCount} subcategories` : ""}`;

  return (
    <OverviewCard
      title={title}
      description={description}
      chips={chips}
      totalCount={total > 0 ? total : undefined}
      manageHref={`/settings/spending/categories?tab=${variant}`}
      emptyTitle={variant === "expense" ? "No expense categories yet" : "No income sources yet"}
      emptyDescription={
        variant === "expense"
          ? "Create categories to organize cash transactions."
          : "Create sources to categorize incoming cash flows."
      }
      emptyCtaLabel={variant === "expense" ? "Add category" : "Add source"}
      isLoading={isLoading}
    />
  );
}
