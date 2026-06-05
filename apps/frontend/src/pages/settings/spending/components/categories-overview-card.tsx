import { useMemo } from "react";

import { useTaxonomy } from "@/hooks/use-taxonomies";
import type { TaxonomyCategory } from "@/lib/types";

import { OverviewCard, type OverviewChip } from "./overview-card";

interface Props {
  variant: "expense" | "income" | "savings";
}

const TAXONOMY_ID = {
  expense: "spending_categories",
  income: "income_sources",
  savings: "savings_categories",
} as const;

export function CategoriesOverviewCard({ variant }: Props) {
  const taxonomyId = TAXONOMY_ID[variant];
  const { data, isLoading } = useTaxonomy(taxonomyId);

  const { chips, topCount, subCount } = useMemo(() => {
    const cats = (data?.categories ?? []) as TaxonomyCategory[];
    const top = cats.filter((c) => !c.parentId).sort((a, b) => a.sortOrder - b.sortOrder);
    const sub = cats.filter((c) => c.parentId);
    // Use number of children as a rough "weight" so the distribution bar
    // visually emphasizes top-level categories with more subcategories.
    const items: OverviewChip[] = top.map((c) => {
      const childCount = sub.filter((s) => s.parentId === c.id).length;
      return {
        id: c.id,
        name: c.name,
        color: c.color,
        value: Math.max(1, childCount),
      };
    });
    return {
      chips: items,
      topCount: top.length,
      subCount: sub.length,
    };
  }, [data?.categories]);

  const title =
    variant === "expense"
      ? "Expense categories"
      : variant === "income"
        ? "Income sources"
        : "Savings categories";
  const description =
    topCount === 0
      ? variant === "expense"
        ? "Group transactions into expense buckets."
        : variant === "income"
          ? "Group transactions into income sources."
          : "Group saved cash flows."
      : variant === "expense"
        ? `${topCount} top-level · ${subCount} subcategories`
        : variant === "income"
          ? `${topCount} sources${subCount > 0 ? ` · ${subCount} subcategories` : ""}`
          : `${topCount} destinations${subCount > 0 ? ` · ${subCount} subcategories` : ""}`;
  const tab = variant === "savings" ? "savings" : variant;

  return (
    <OverviewCard
      title={title}
      description={description}
      chips={chips}
      manageHref={`/settings/spending/categories?tab=${tab}`}
      emptyTitle={
        variant === "expense"
          ? "No expense categories yet"
          : variant === "income"
            ? "No income sources yet"
            : "No savings categories yet"
      }
      emptyDescription={
        variant === "expense"
          ? "Create categories to organize cash transactions."
          : variant === "income"
            ? "Create sources to categorize incoming cash flows."
            : "Create categories to organize saved cash flows."
      }
      emptyCtaLabel={variant === "income" ? "Add source" : "Add category"}
      isLoading={isLoading}
      showDistribution
    />
  );
}
