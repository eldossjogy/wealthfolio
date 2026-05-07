export interface BudgetConfig {
  id: string;
  monthlySpendingTarget: string;
  monthlyIncomeTarget: string;
  currency: string;
  createdAt: string;
  updatedAt: string;
}

export interface BudgetAllocation {
  id: string;
  budgetConfigId: string;
  taxonomyId: string;
  categoryId: string;
  amount: string;
  createdAt: string;
  updatedAt: string;
}

export interface BudgetSnapshot {
  config: BudgetConfig;
  allocations: BudgetAllocation[];
}

export interface UpdateBudgetConfig {
  monthlySpendingTarget?: string;
  monthlyIncomeTarget?: string;
  currency?: string;
}
