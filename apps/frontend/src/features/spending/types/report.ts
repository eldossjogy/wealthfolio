export interface PeriodSummary {
  income: number;
  outflow: number;
  net: number;
  count: number;
}

export interface CategoryBreakdownRow {
  taxonomyId: string;
  categoryId: string;
  amount: number;
  count: number;
}

export interface DayBucket {
  date: string;
  income: number;
  outflow: number;
}

export interface MonthlyReport {
  current: PeriodSummary;
  prior: PeriodSummary;
  spendingBreakdown: CategoryBreakdownRow[];
  incomeBreakdown: CategoryBreakdownRow[];
  byDay: DayBucket[];
}

export interface ReportRequest {
  startDate: string;
  endDate: string;
  accountIds?: string[];
}
