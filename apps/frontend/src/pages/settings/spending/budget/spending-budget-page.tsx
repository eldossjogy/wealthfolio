import { useMemo, useState } from "react";
import { Navigate } from "react-router-dom";

import {
  Button,
  Icons,
  Separator,
  Skeleton,
  Tabs,
  TabsContent,
  TabsList,
  TabsTrigger,
} from "@wealthfolio/ui";
import { useTaxonomy } from "@/hooks/use-taxonomies";
import type { TaxonomyCategory } from "@/lib/types";

import { AllocationFormDialog } from "@/features/spending/components/allocation-form-dialog";
import {
  AllocationList,
  type AllocationWithCategory,
} from "@/features/spending/components/allocation-list";
import { BudgetTargetForm } from "@/features/spending/components/budget-target-form";
import { useBudget, useBudgetMutations } from "@/features/spending/hooks/use-budget";
import { useSpendingSettings } from "@/features/spending/hooks/use-spending-settings";

import { SettingsHeader } from "../../settings-header";
import { SpendingBackLink } from "../components/spending-back-link";

const SPENDING_TAXONOMY = "spending_categories";
const INCOME_TAXONOMY = "income_sources";

export default function SpendingBudgetPage() {
  const { isEnabled, isLoading: settingsLoading } = useSpendingSettings();
  const { data: budget, isLoading: budgetLoading } = useBudget();
  const { updateConfig, upsertAllocation, removeAllocation } = useBudgetMutations();
  const spending = useTaxonomy(SPENDING_TAXONOMY);
  const income = useTaxonomy(INCOME_TAXONOMY);

  const [activeTab, setActiveTab] = useState<"expense" | "income">("expense");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingAllocation, setEditingAllocation] = useState<AllocationWithCategory | undefined>();

  const isLoading = budgetLoading || spending.isLoading || income.isLoading;
  const currency = budget?.config?.currency ?? "USD";

  const topLevelByTaxonomy = useMemo(() => {
    const filterTop = (cats: TaxonomyCategory[] | undefined) =>
      (cats ?? []).filter((c) => !c.parentId).sort((a, b) => a.sortOrder - b.sortOrder);
    return {
      [SPENDING_TAXONOMY]: filterTop(spending.data?.categories),
      [INCOME_TAXONOMY]: filterTop(income.data?.categories),
    };
  }, [spending.data?.categories, income.data?.categories]);

  const allCategoriesById = useMemo(() => {
    const map = new Map<string, TaxonomyCategory>();
    (spending.data?.categories ?? []).forEach((c) => map.set(c.id, c));
    (income.data?.categories ?? []).forEach((c) => map.set(c.id, c));
    return map;
  }, [spending.data?.categories, income.data?.categories]);

  const { expenseAllocations, incomeAllocations } = useMemo(() => {
    const allocs = budget?.allocations ?? [];
    const annotate = (a: (typeof allocs)[number]): AllocationWithCategory => {
      const cat = allCategoriesById.get(a.categoryId);
      return {
        ...a,
        categoryName: cat?.name ?? a.categoryId,
        categoryColor: cat?.color ?? null,
      };
    };
    return {
      expenseAllocations: allocs.filter((a) => a.taxonomyId === SPENDING_TAXONOMY).map(annotate),
      incomeAllocations: allocs.filter((a) => a.taxonomyId === INCOME_TAXONOMY).map(annotate),
    };
  }, [budget?.allocations, allCategoriesById]);

  const spendingTarget = parseFloat(budget?.config?.monthlySpendingTarget ?? "0") || 0;
  const incomeTarget = parseFloat(budget?.config?.monthlyIncomeTarget ?? "0") || 0;
  const sumAllocations = (allocs: AllocationWithCategory[]) =>
    allocs.reduce((sum, a) => sum + (parseFloat(a.amount) || 0), 0);
  const unallocatedSpending = Math.max(0, spendingTarget - sumAllocations(expenseAllocations));
  const unallocatedIncome = Math.max(0, incomeTarget - sumAllocations(incomeAllocations));

  if (!settingsLoading && !isEnabled) {
    return <Navigate to="/settings/spending" replace />;
  }

  if (isLoading) {
    return (
      <div className="space-y-4">
        <Skeleton className="h-12" />
        <Skeleton className="h-12" />
        <Skeleton className="h-32" />
      </div>
    );
  }

  const activeTaxonomyId = activeTab === "expense" ? SPENDING_TAXONOMY : INCOME_TAXONOMY;
  const currentCategories = topLevelByTaxonomy[activeTaxonomyId];
  const currentAllocations = activeTab === "expense" ? expenseAllocations : incomeAllocations;

  const handleAddAllocation = () => {
    setEditingAllocation(undefined);
    setDialogOpen(true);
  };

  const handleEditAllocation = (allocation: AllocationWithCategory) => {
    setEditingAllocation(allocation);
    setDialogOpen(true);
  };

  const handleSaveAllocation = (categoryId: string, amount: string) => {
    upsertAllocation.mutate({
      taxonomyId: activeTaxonomyId,
      categoryId,
      amount,
    });
  };

  const handleDeleteAllocation = (allocationId: string) => {
    removeAllocation.mutate(allocationId);
  };

  const hasConfig = !!budget?.config;

  return (
    <>
      <div className="space-y-6">
        <SpendingBackLink />
        <SettingsHeader
          heading="Budget defaults"
          text="Default monthly spending and income targets, and how they split across categories. Used as the baseline whenever a month has no override."
          backTo="/settings/spending"
        />

        <BudgetTargetForm
          config={budget?.config ?? null}
          currency={currency}
          onSave={(patch) => updateConfig.mutate(patch)}
          isPending={updateConfig.isPending}
        />

        {hasConfig && (
          <>
            <Separator />
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <h3 className="text-base font-semibold">Default category allocations</h3>
                <Button onClick={handleAddAllocation} size="sm">
                  <Icons.Plus className="mr-2 h-4 w-4" />
                  Add allocation
                </Button>
              </div>

              <Tabs
                value={activeTab}
                onValueChange={(v) => setActiveTab(v as "expense" | "income")}
              >
                <TabsList>
                  <TabsTrigger value="expense">Spending</TabsTrigger>
                  <TabsTrigger value="income">Income</TabsTrigger>
                </TabsList>

                <TabsContent value="expense" className="mt-4">
                  <AllocationList
                    allocations={expenseAllocations}
                    unallocated={unallocatedSpending}
                    currency={currency}
                    onEdit={handleEditAllocation}
                    onDelete={handleDeleteAllocation}
                    isDeleting={removeAllocation.isPending}
                  />
                </TabsContent>

                <TabsContent value="income" className="mt-4">
                  <AllocationList
                    allocations={incomeAllocations}
                    unallocated={unallocatedIncome}
                    currency={currency}
                    onEdit={handleEditAllocation}
                    onDelete={handleDeleteAllocation}
                    isDeleting={removeAllocation.isPending}
                  />
                </TabsContent>
              </Tabs>
            </div>
          </>
        )}
      </div>

      <AllocationFormDialog
        open={dialogOpen}
        onClose={() => setDialogOpen(false)}
        onSave={handleSaveAllocation}
        categories={currentCategories}
        existingAllocations={currentAllocations}
        editingAllocation={editingAllocation}
        isIncome={activeTab === "income"}
        isPending={upsertAllocation.isPending}
      />
    </>
  );
}
