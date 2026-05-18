import { useCallback, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";

import { useTaxonomy } from "@/hooks/use-taxonomies";
import { useAccounts } from "@/hooks/use-accounts";
import { useSettingsContext } from "@/lib/settings-provider";

import {
  AnimatedToggleGroup,
  Page,
  PageContent,
  PageHeader,
  usePersistentState,
} from "@wealthfolio/ui";

import { CategoryTransactionsSheet } from "../components/reports/category-transactions-sheet";
import { HeatmapCellSheet } from "../components/reports/heatmap-cell-sheet";
import { StageNav, type InsightsStage } from "../components/reports/insights/stage-nav";
import { WhatChangedStage } from "../components/reports/insights/what-changed-stage";
import { WhenWhereStage } from "../components/reports/insights/when-where-stage";
import { WhereIAmStage } from "../components/reports/insights/where-i-am-stage";
import { useBudget } from "../hooks/use-budget";
import { useCashActivities } from "../hooks/use-cash-activities";
import { useMonthlyHistory } from "../hooks/use-monthly-history";
import { useEventSpendingSummaries } from "../hooks/use-spending-events";
import { useSpendingReport } from "../hooks/use-spending-report";
import {
  DEFAULT_REPORTS_PERIOD,
  REPORTS_PERIODS,
  comparisonRange,
  periodToReportsRange,
  rangeToReportRequest,
  type ReportsPeriod,
} from "../lib/reports-period";

const SPENDING_TAXONOMY = "spending_categories";
const PERIOD_STORAGE_KEY = "spending-insights-period";
const STAGE_STORAGE_KEY = "spending-insights-stage";
const EMPTY_TAXONOMY: never[] = [];
/** Heatmap window — last 12 weeks regardless of selected period. */
const HEATMAP_WEEKS = 12;
const DAILY_GRANULARITY_THRESHOLD_DAYS = 35;
const HEATMAP_DAY_NAMES = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] as const;

/**
 * Spending insights — narrative-first, three-stage page.
 *
 *   Where I am   — pace card + spent + cashflow + breakdown table
 *   What changed — period-vs-period headline + sparklines + delta table
 *   When & where — weekday-hour heatmap + events headline + per-event cards
 *
 * Owns period + comparison + stage state at the top; each stage receives the
 * data it needs. Data hooks run unconditionally so switching stages is instant.
 */
const PERIOD_LABELS: Record<ReportsPeriod, string> = {
  "1M": "1M",
  "3M": "3M",
  "6M": "6M",
  YTD: "YTD",
  "1Y": "1Y",
};

export default function SpendingInsightsPage() {
  const navigate = useNavigate();
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const [period, setPeriod] = usePersistentState<ReportsPeriod>(
    PERIOD_STORAGE_KEY,
    DEFAULT_REPORTS_PERIOD,
  );
  const [stage, setStage] = usePersistentState<InsightsStage>(STAGE_STORAGE_KEY, "where");

  const range = useMemo(() => periodToReportsRange(period), [period]);
  const taxonomy = useTaxonomy(SPENDING_TAXONOMY);
  const { data: budget, isLoading: isBudgetLoading } = useBudget();
  const { accounts = [] } = useAccounts({ filterActive: false });

  const currentRequest = useMemo(() => rangeToReportRequest(range), [range]);
  const { data: currentReport, isLoading: isCurrentLoading } = useSpendingReport(currentRequest);

  // Comparison is always the immediately preceding window of equal size.
  const priorRange = useMemo(() => comparisonRange(range, "prior"), [range]);
  const priorRequest = useMemo(
    () => (priorRange ? rangeToReportRequest(priorRange) : currentRequest),
    [priorRange, currentRequest],
  );
  const { data: priorReport, isLoading: isPriorLoading } = useSpendingReport(priorRequest, true);

  const { months, isLoading: isHistoryLoading } = useMonthlyHistory(range);

  // 12-week activity window for the weekday × hour heatmap.
  const heatmapRequest = useMemo(() => {
    const end = new Date();
    end.setHours(23, 59, 59, 999);
    const start = new Date(end);
    start.setDate(end.getDate() - HEATMAP_WEEKS * 7);
    start.setHours(0, 0, 0, 0);
    return { startDate: start.toISOString(), endDate: end.toISOString() };
  }, []);
  const { data: heatmapActivities = [] } = useCashActivities(heatmapRequest);

  const eventsRequest = useMemo(
    () => ({ startDate: range.start.toISOString(), endDate: range.end.toISOString() }),
    [range],
  );
  const { data: events = [] } = useEventSpendingSummaries(eventsRequest);

  // Header context — Mon D – Mon D, YYYY · N tx
  const contextLabel = useMemo(() => {
    const fmt = new Intl.DateTimeFormat(undefined, { month: "short", day: "numeric" });
    const sameYear = range.start.getFullYear() === range.end.getFullYear();
    const yearFmt = new Intl.DateTimeFormat(undefined, { year: "numeric" });
    const startStr = fmt.format(range.start);
    const endStr = fmt.format(range.end);
    const yearStr = sameYear ? `, ${yearFmt.format(range.end)}` : "";
    const txCount = currentReport?.current.count;
    const txLabel = txCount != null ? ` · ${txCount.toLocaleString()} tx` : "";
    return `${startStr} – ${endStr}${yearStr}${txLabel}`;
  }, [range, currentReport]);

  const onJumpToBreakdown = useCallback(() => {
    const el = document.getElementById("breakdown");
    if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
  }, []);

  // Click-through sheet for category transactions
  const [activeCategoryId, setActiveCategoryId] = useState<string | null>(null);
  const handleCategoryClick = useCallback((categoryId: string) => {
    setActiveCategoryId(categoryId);
  }, []);
  const activeCategory = useMemo(
    () =>
      activeCategoryId
        ? (taxonomy.data?.categories.find((c) => c.id === activeCategoryId) ?? null)
        : null,
    [activeCategoryId, taxonomy.data?.categories],
  );

  // Click-through sheet for heatmap cells (weekday × hour)
  const [heatmapCell, setHeatmapCell] = useState<{ weekday: number; hour: number } | null>(null);
  const handleHeatmapCellClick = useCallback((weekday: number, hour: number) => {
    setHeatmapCell({ weekday, hour });
  }, []);
  const heatmapCellActivities = useMemo(() => {
    if (!heatmapCell) return [];
    return heatmapActivities.filter((a) => {
      const d = new Date(a.activityDate);
      if (isNaN(d.getTime())) return false;
      const weekday = (d.getDay() + 6) % 7;
      return weekday === heatmapCell.weekday && d.getHours() === heatmapCell.hour;
    });
  }, [heatmapActivities, heatmapCell]);

  const useDailyForHistory = range.days <= DAILY_GRANULARITY_THRESHOLD_DAYS;
  const taxonomyCategories = taxonomy.data?.categories ?? EMPTY_TAXONOMY;
  const accountTypeById = useMemo(
    () => new Map(accounts.map((account) => [account.id, account.accountType])),
    [accounts],
  );

  const periodToggle = (
    <AnimatedToggleGroup
      variant="secondary"
      size="xs"
      items={REPORTS_PERIODS.map((p) => ({ value: p, label: PERIOD_LABELS[p] }))}
      value={period}
      onValueChange={setPeriod}
    />
  );

  return (
    <Page>
      <PageHeader
        heading="Spending Insight"
        text="Patterns, comparisons, and anomalies for your spending."
        onBack={() => {
          if (window.history.length > 1) navigate(-1);
          else navigate("/dashboard?tab=spending");
        }}
        actions={periodToggle}
      />
      <PageContent className="space-y-5">
        <StageNav stage={stage} onStageChange={setStage} contextLabel={contextLabel} />

        {stage === "where" && (
          <WhereIAmStage
            range={range}
            currentReport={currentReport}
            priorReport={priorReport}
            months={months}
            taxonomyCategories={taxonomyCategories}
            budget={budget}
            currency={baseCurrency}
            isLoading={isCurrentLoading || isBudgetLoading}
            onJumpToBreakdown={onJumpToBreakdown}
            onCategoryClick={handleCategoryClick}
          />
        )}

        {stage === "changed" && (
          <WhatChangedStage
            range={range}
            currentReport={currentReport}
            priorReport={priorReport}
            months={months}
            taxonomyCategories={taxonomyCategories}
            currency={baseCurrency}
            isLoading={
              isCurrentLoading || isPriorLoading || (!useDailyForHistory && isHistoryLoading)
            }
            onCategoryClick={handleCategoryClick}
          />
        )}

        {stage === "when" && (
          <WhenWhereStage
            heatmapActivities={heatmapActivities}
            accountTypeById={accountTypeById}
            events={events}
            taxonomyCategories={taxonomyCategories}
            currency={baseCurrency}
            rangeStart={range.start}
            rangeEnd={range.end}
            onHeatmapCellClick={handleHeatmapCellClick}
          />
        )}
      </PageContent>

      <CategoryTransactionsSheet
        open={!!activeCategory}
        onOpenChange={(open) => {
          if (!open) setActiveCategoryId(null);
        }}
        category={activeCategory}
        taxonomyCategories={taxonomyCategories}
        rangeStart={range.start}
        rangeEnd={range.end}
        currency={baseCurrency}
      />

      <HeatmapCellSheet
        open={!!heatmapCell}
        onOpenChange={(open) => {
          if (!open) setHeatmapCell(null);
        }}
        activities={heatmapCellActivities}
        dayLabel={heatmapCell ? HEATMAP_DAY_NAMES[heatmapCell.weekday] : null}
        hour={heatmapCell?.hour ?? null}
        currency={baseCurrency}
      />
    </Page>
  );
}
