import { useMemo, useState } from "react";
import { Navigate } from "react-router-dom";

import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  EmptyPlaceholder,
  Icons,
  Skeleton,
} from "@wealthfolio/ui";
import { useTaxonomy } from "@/hooks/use-taxonomies";
import type { TaxonomyCategory } from "@/lib/types";

import { RuleEditModal } from "@/features/spending/components/rule-edit-modal";
import { RuleItem, type RuleCategoryMeta } from "@/features/spending/components/rule-item";
import { RulePresetPicker } from "@/features/spending/components/rule-preset-picker";
import type {
  RuleFormCategoryOption,
  RuleFormValues,
} from "@/features/spending/components/rule-form";
import {
  useCategorizationRuleMutations,
  useCategorizationRules,
} from "@/features/spending/hooks/use-categorization-rules";
import { useSpendingSettings } from "@/features/spending/hooks/use-spending-settings";
import type { CategorizationRule } from "@/features/spending/types/rule";

import { SettingsHeader } from "../../settings-header";
import { SpendingBackLink } from "../components/spending-back-link";

const SPENDING_TAXONOMY = "spending_categories";
const INCOME_TAXONOMY = "income_sources";

export default function SpendingRulesPage() {
  const { isEnabled, isLoading: settingsLoading } = useSpendingSettings();
  const { data: rules = [], isLoading: rulesLoading } = useCategorizationRules();
  const spending = useTaxonomy(SPENDING_TAXONOMY);
  const income = useTaxonomy(INCOME_TAXONOMY);
  const { create, update, remove, rerun } = useCategorizationRuleMutations();

  const [visibleModal, setVisibleModal] = useState(false);
  const [selectedRule, setSelectedRule] = useState<CategorizationRule | undefined>();

  const isLoading = rulesLoading || spending.isLoading || income.isLoading;

  const { categoryOptions, categoryMeta } = useMemo(() => {
    const buildOptions = (taxonomyId: string, cats: TaxonomyCategory[]) => {
      const byId = new Map(cats.map((c) => [c.id, c]));
      return cats
        .slice()
        .sort((a, b) => a.sortOrder - b.sortOrder)
        .map((c) => {
          const parent = c.parentId ? byId.get(c.parentId) : null;
          return {
            value: `${taxonomyId}:${c.id}`,
            label: c.name,
            taxonomyId,
            categoryId: c.id,
            color: c.color,
            parentName: parent?.name ?? null,
          } satisfies RuleFormCategoryOption;
        });
    };
    const opts: RuleFormCategoryOption[] = [
      ...buildOptions(SPENDING_TAXONOMY, spending.data?.categories ?? []),
      ...buildOptions(INCOME_TAXONOMY, income.data?.categories ?? []),
    ];
    const meta: Record<string, RuleCategoryMeta> = {};
    opts.forEach((o) => {
      meta[o.categoryId] = {
        name: o.label,
        color: o.color ?? null,
        parentName: o.parentName ?? null,
      };
    });
    return { categoryOptions: opts, categoryMeta: meta };
  }, [spending.data?.categories, income.data?.categories]);

  if (!settingsLoading && !isEnabled) {
    return <Navigate to="/settings/spending" replace />;
  }

  const handleAddRule = () => {
    setSelectedRule(undefined);
    setVisibleModal(true);
  };

  const handleEditRule = (rule: CategorizationRule) => {
    setSelectedRule(rule);
    setVisibleModal(true);
  };

  const handleDeleteRule = (rule: CategorizationRule) => {
    remove.mutate(rule.id);
  };

  const handleSave = (values: RuleFormValues) => {
    if (selectedRule) {
      update.mutate(
        {
          id: selectedRule.id,
          patch: {
            name: values.name,
            pattern: values.pattern,
            matchType: values.matchType,
            taxonomyId: values.taxonomyId || null,
            categoryId: values.categoryId || null,
            activityType: values.activityType || null,
            priority: values.priority,
            isGlobal: values.isGlobal,
          },
        },
        {
          onSuccess: () => setVisibleModal(false),
        },
      );
    } else {
      create.mutate(
        {
          name: values.name,
          pattern: values.pattern,
          matchType: values.matchType,
          taxonomyId: values.taxonomyId || null,
          categoryId: values.categoryId || null,
          activityType: values.activityType || null,
          priority: values.priority,
          isGlobal: values.isGlobal,
        },
        {
          onSuccess: () => setVisibleModal(false),
        },
      );
    }
  };

  const sortedRules = [...rules].sort((a, b) => b.priority - a.priority);

  return (
    <>
      <div className="space-y-6">
        <SpendingBackLink />
        <SettingsHeader
          heading="Categorization rules"
          text="Automatically tag activities by transaction-name patterns. When multiple rules match, the higher-priority one wins."
          backTo="/settings/spending"
        >
          <div className="flex items-center gap-2">
            <DropdownMenu>
              <DropdownMenuTrigger asChild>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={rerun.isPending || rules.length === 0}
                >
                  {rerun.isPending ? (
                    <Icons.Spinner className="mr-2 h-3.5 w-3.5 animate-spin" />
                  ) : (
                    <Icons.Refresh className="mr-2 h-3.5 w-3.5" />
                  )}
                  Re-run rules
                  <Icons.ChevronDown className="ml-2 h-3.5 w-3.5 opacity-60" />
                </Button>
              </DropdownMenuTrigger>
              <DropdownMenuContent align="end" className="w-72">
                <DropdownMenuItem
                  onClick={() => rerun.mutate(true)}
                  className="flex-col items-start gap-0.5"
                >
                  <span className="text-sm font-medium">Categorize uncategorized</span>
                  <span className="text-muted-foreground text-xs">
                    Apply rules only to activities without a category. Safe — won&apos;t overwrite.
                  </span>
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  onClick={() => rerun.mutate(false)}
                  className="flex-col items-start gap-0.5"
                >
                  <span className="text-sm font-medium">Reclassify all</span>
                  <span className="text-muted-foreground text-xs">
                    Re-apply rules to every activity. Overwrites existing categorizations.
                  </span>
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
            <Button size="sm" className="sm:hidden" onClick={handleAddRule} aria-label="Add rule">
              <Icons.Plus className="h-3.5 w-3.5" />
            </Button>
            <Button size="sm" className="hidden sm:inline-flex" onClick={handleAddRule}>
              <Icons.Plus className="mr-2 h-3.5 w-3.5" />
              Add rule
            </Button>
          </div>
        </SettingsHeader>

        {isLoading ? (
          <div className="space-y-4">
            <Skeleton className="h-12 w-full" />
            <Skeleton className="h-12 w-full" />
            <Skeleton className="h-12 w-full" />
          </div>
        ) : sortedRules.length === 0 ? (
          <div className="space-y-6">
            <div className="bg-muted/30 rounded-lg border p-4">
              <RulePresetPicker />
            </div>
            <EmptyPlaceholder>
              <EmptyPlaceholder.Icon name="ListFilter" />
              <EmptyPlaceholder.Title>No rules yet</EmptyPlaceholder.Title>
              <EmptyPlaceholder.Description>
                Pick a country preset above to seed common merchant rules, or create your own rules
                to automatically assign categories during import or on activity create.
              </EmptyPlaceholder.Description>
              <Button variant="outline" onClick={handleAddRule}>
                <Icons.Plus className="mr-2 h-4 w-4" />
                Add rule manually
              </Button>
            </EmptyPlaceholder>
          </div>
        ) : (
          <div className="space-y-4">
            <div className="bg-muted/30 rounded-md border p-3">
              <RulePresetPicker compact />
            </div>
            <div className="divide-border divide-y rounded-md border">
              {sortedRules.map((rule) => (
                <RuleItem
                  key={rule.id}
                  rule={rule}
                  categoryMeta={categoryMeta}
                  onEdit={handleEditRule}
                  onDelete={handleDeleteRule}
                />
              ))}
            </div>
          </div>
        )}
      </div>

      <RuleEditModal
        open={visibleModal}
        onClose={() => setVisibleModal(false)}
        rule={selectedRule}
        categoryOptions={categoryOptions}
        onSave={handleSave}
        isLoading={create.isPending || update.isPending}
      />
    </>
  );
}
