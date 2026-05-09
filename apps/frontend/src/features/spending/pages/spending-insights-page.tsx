import { useCallback, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";

import { useTaxonomy } from "@/hooks/use-taxonomies";
import { QueryKeys } from "@/lib/query-keys";
import { useSettingsContext } from "@/lib/settings-provider";

import { AnimatedToggleGroup, Icons, usePersistentState } from "@wealthfolio/ui";

import { getEventSpendingSummaries, listEventTypes } from "../adapters/events";
import {
  CashflowDivergingBars,
  type CashflowPoint,
} from "../components/reports/cashflow-diverging-bars";
import { CategoryHierarchyTable } from "../components/reports/category-hierarchy-table";
import { CategorySparklineGrid } from "../components/reports/category-sparkline-grid";
import {
  BudgetStatusHero,
  CashflowHero,
  PeriodSummaryHero,
} from "../components/reports/hero-strip";
import { NotableChangesCard } from "../components/reports/notable-changes-card";
import { SpendingRhythmHeatmap } from "../components/reports/spending-rhythm-heatmap";
import { EventCategoryTreemap } from "../components/event-category-treemap";
import { EventTimeline } from "../components/event-timeline";
import { useBudget } from "../hooks/use-budget";
import { useCashActivities } from "../hooks/use-cash-activities";
import { useMonthlyHistory } from "../hooks/use-monthly-history";
import { useSpendingReport } from "../hooks/use-spending-report";
import { FOREST_THEME } from "../lib/theme";
import {
  DEFAULT_COMPARISON,
  DEFAULT_REPORTS_PERIOD,
  REPORTS_PERIODS,
  comparisonRange,
  periodToReportsRange,
  rangeToReportRequest,
  type ComparisonMode,
  type ReportsPeriod,
} from "../lib/reports-period";
import type { EventSpendingSummary, EventType } from "../types/event";

const SPENDING_TAXONOMY = "spending_categories";
const PERIOD_STORAGE_KEY = "spending-insights-period";
const COMPARISON_STORAGE_KEY = "spending-insights-comparison";
/** Range threshold for switching cashflow + sparklines to daily granularity. */
const DAILY_GRANULARITY_THRESHOLD_DAYS = 35;

const PERIOD_LABELS: Record<ReportsPeriod, string> = {
  "1M": "1M",
  "3M": "3M",
  "6M": "6M",
  YTD: "YTD",
  "1Y": "1Y",
};

const COMPARISON_LABELS: Record<ComparisonMode, string> = {
  prior: "Prior",
  yoy: "YoY",
  none: "None",
};

const COMPARISON_OPTIONS: ComparisonMode[] = ["prior", "yoy", "none"];

/**
 * Spending insights — single-page dashboard mirroring the holdings-insights pattern.
 *
 * Owns period + comparison state at the top, then flows widgets vertically:
 *   1. Hero strip (period summary, budget status, cashflow KPIs)
 *   2. Cashflow over time (adaptive bucket granularity)
 *   3. Category trends (sparklines, adaptive granularity)
 *   4. Breakdown table (the primary working canvas)
 *   5. Patterns (rhythm heatmap + notable changes, side-by-side)
 *   6. Events (only renders if user has events in the window)
 */
export default function SpendingInsightsPage() {
  const navigate = useNavigate();
  const { settings } = useSettingsContext();
  const baseCurrency = settings?.baseCurrency ?? "USD";

  const [period, setPeriod] = usePersistentState<ReportsPeriod>(
    PERIOD_STORAGE_KEY,
    DEFAULT_REPORTS_PERIOD,
  );
  const [comparison, setComparison] = usePersistentState<ComparisonMode>(
    COMPARISON_STORAGE_KEY,
    DEFAULT_COMPARISON,
  );

  const range = useMemo(() => periodToReportsRange(period), [period]);

  const taxonomy = useTaxonomy(SPENDING_TAXONOMY);
  const { data: budget, isLoading: isBudgetLoading } = useBudget();

  // Current + prior reports drive the breakdown table, hero KPIs, and the
  // sparkline % chip. Both run unconditionally so widgets can render without
  // waterfalling.
  const currentRequest = useMemo(() => rangeToReportRequest(range), [range]);
  const { data: currentReport, isLoading: isCurrentLoading } = useSpendingReport(currentRequest);

  const priorRange = useMemo(() => comparisonRange(range, comparison), [range, comparison]);
  const priorRequest = useMemo(
    () => (priorRange ? rangeToReportRequest(priorRange) : currentRequest),
    [priorRange, currentRequest],
  );
  const { data: priorReport, isLoading: isPriorLoading } = useSpendingReport(
    priorRequest,
    priorRange != null,
  );

  // Monthly buckets — used for the cashflow chart (when range is multi-month)
  // and the cashflow hero KPIs.
  const { months, isLoading: isHistoryLoading } = useMonthlyHistory(range);

  // Spending rhythm heatmap is always last 4 weeks regardless of period.
  const heatmapRequest = useMemo(() => {
    const end = new Date();
    end.setHours(23, 59, 59, 999);
    const start = new Date(end);
    start.setDate(end.getDate() - 4 * 7);
    start.setHours(0, 0, 0, 0);
    return { startDate: start.toISOString(), endDate: end.toISOString() };
  }, []);
  const { data: heatmapActivities = [] } = useCashActivities(heatmapRequest);

  // Adaptive granularity — short windows get day-bucketed cashflow + sparklines.
  const useDaily = range.days <= DAILY_GRANULARITY_THRESHOLD_DAYS;
  const cashflowPoints: CashflowPoint[] = useMemo(() => {
    if (useDaily) {
      return (currentReport?.byDay ?? []).map((b) => ({
        label: b.date.slice(8),
        income: b.income,
        outflow: b.outflow,
      }));
    }
    return months.map((m) => ({
      label: m.label,
      income: m.report?.current.income ?? 0,
      outflow: m.report?.current.outflow ?? 0,
    }));
  }, [useDaily, currentReport, months]);

  // Events — only fetched/rendered if the user actually has events in scope.
  const eventsRequest = useMemo(
    () => ({ startDate: range.start.toISOString(), endDate: range.end.toISOString() }),
    [range],
  );
  const { data: eventSummaries = [] } = useQuery<EventSpendingSummary[]>({
    queryKey: [QueryKeys.SPENDING_EVENTS, "summaries", eventsRequest],
    queryFn: () => getEventSpendingSummaries(eventsRequest),
  });
  const { data: eventTypes = [] } = useQuery<EventType[]>({
    queryKey: [QueryKeys.SPENDING_EVENT_TYPES],
    queryFn: listEventTypes,
    enabled: eventSummaries.length > 0,
  });
  const eventCurrency = eventSummaries[0]?.currency ?? baseCurrency;

  // Event-type filter for the timeline — multi-select set of type ids.
  const [selectedEventTypes, setSelectedEventTypes] = useState<Set<string>>(() => new Set());
  const toggleEventType = useCallback((eventTypeId: string) => {
    setSelectedEventTypes((prev) => {
      const next = new Set(prev);
      if (next.has(eventTypeId)) next.delete(eventTypeId);
      else next.add(eventTypeId);
      return next;
    });
  }, []);
  const filteredEventSummaries = useMemo(
    () =>
      selectedEventTypes.size === 0
        ? eventSummaries
        : eventSummaries.filter((s) => selectedEventTypes.has(s.eventTypeId)),
    [eventSummaries, selectedEventTypes],
  );

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-6 px-4 pb-6 pt-4 md:px-6 lg:px-8 lg:pb-8">
      {/* Header — back link + title + period/comparison toggles */}
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <button
            type="button"
            onClick={() => navigate(-1)}
            className="text-muted-foreground hover:text-foreground mb-1 inline-flex items-center gap-1 text-xs underline-offset-4 hover:underline"
          >
            <Icons.ArrowLeft className="h-3.5 w-3.5" />
            Back
          </button>
          <h1 className="text-2xl font-bold tracking-tight">Spending insights</h1>
          <p className="text-muted-foreground text-sm">
            Patterns, trends, and deeper analysis of your spending.
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <div className="flex flex-col gap-1">
            <span className="text-muted-foreground/70 text-[10px] font-semibold uppercase tracking-wide">
              Period
            </span>
            <AnimatedToggleGroup
              variant="secondary"
              size="xs"
              items={REPORTS_PERIODS.map((p) => ({ value: p, label: PERIOD_LABELS[p] }))}
              value={period}
              onValueChange={setPeriod}
            />
          </div>
          <div className="flex flex-col gap-1">
            <span className="text-muted-foreground/70 text-[10px] font-semibold uppercase tracking-wide">
              Compare to
            </span>
            <AnimatedToggleGroup
              variant="secondary"
              size="xs"
              items={COMPARISON_OPTIONS.map((m) => ({ value: m, label: COMPARISON_LABELS[m] }))}
              value={comparison}
              onValueChange={setComparison}
            />
          </div>
        </div>
      </div>

      {/* Hero strip — at-a-glance period status (3-card layout) */}
      <div className="grid gap-3 lg:grid-cols-3">
        <PeriodSummaryHero
          spent={currentReport?.current.outflow ?? 0}
          days={range.days}
          months={range.months}
          breakdown={currentReport?.spendingBreakdown ?? []}
          taxonomyCategories={taxonomy.data?.categories ?? []}
          currency={baseCurrency}
          isLoading={isCurrentLoading}
        />
        <BudgetStatusHero
          spent={currentReport?.current.outflow ?? 0}
          monthsInRange={range.months}
          budget={budget}
          currency={baseCurrency}
          isLoading={isCurrentLoading || isBudgetLoading}
        />
        <CashflowHero months={months} currency={baseCurrency} isLoading={isHistoryLoading} />
      </div>

      {/* Cashflow over time */}
      <Section title="Cashflow over time" subtitle="Income above · spending below · net line">
        <CashflowDivergingBars
          points={cashflowPoints}
          currency={baseCurrency}
          isLoading={useDaily ? isCurrentLoading : isHistoryLoading}
        />
      </Section>

      {/* Category trends */}
      <Section title="Category trends" subtitle="Sparkline per top-level category">
        <CategorySparklineGrid
          taxonomyCategories={taxonomy.data?.categories ?? []}
          currency={baseCurrency}
          isLoading={useDaily ? isCurrentLoading : isHistoryLoading}
          priorBreakdown={priorReport?.spendingBreakdown ?? []}
          granularity={useDaily ? "day" : "month"}
          months={useDaily ? undefined : months}
          byDayByCategory={useDaily ? (currentReport?.byDayByCategory ?? []) : undefined}
        />
      </Section>

      {/* Breakdown — the primary working canvas */}
      <Section title="Breakdown" subtitle="Spent, budget progress, change vs prior period">
        <CategoryHierarchyTable
          breakdown={currentReport?.spendingBreakdown ?? []}
          priorBreakdown={priorReport?.spendingBreakdown ?? []}
          allocations={budget?.allocations ?? []}
          taxonomyCategories={taxonomy.data?.categories ?? []}
          currency={baseCurrency}
          isLoading={isCurrentLoading || (comparison !== "none" && isPriorLoading)}
        />
      </Section>

      {/* Patterns row — rhythm heatmap + notable changes */}
      <div className="grid gap-6 lg:grid-cols-2">
        <Section title="Spending rhythm" subtitle="Last 4 weeks · darker = heavier">
          <SpendingRhythmHeatmap
            activities={heatmapActivities}
            weeks={4}
            accent={FOREST_THEME.deep}
            currency={baseCurrency}
          />
        </Section>

        <Section
          title="Notable changes"
          subtitle={
            comparison === "none" ? "Enable a comparison to surface" : "vs comparison period"
          }
        >
          <NotableChangesCard
            current={currentReport}
            prior={comparison === "none" ? undefined : priorReport}
            taxonomyCategories={taxonomy.data?.categories ?? []}
            currency={baseCurrency}
            isLoading={isCurrentLoading || (comparison !== "none" && isPriorLoading)}
          />
        </Section>
      </div>

      {/* Events — only renders if the user has events in scope */}
      {eventSummaries.length > 0 && (
        <>
          <Section title="Categories across events" subtitle="Aggregate share of spend">
            <EventCategoryTreemap events={filteredEventSummaries} currency={eventCurrency} />
          </Section>

          <Section title="Event timeline" subtitle="Each event positioned on its date range">
            <EventTimeline
              events={eventSummaries}
              eventTypes={eventTypes}
              selectedEventTypes={selectedEventTypes}
              onToggleEventType={toggleEventType}
              periodDateRange={{
                startDate: range.start.toISOString().slice(0, 10),
                endDate: range.end.toISOString().slice(0, 10),
              }}
            />
          </Section>
        </>
      )}
    </div>
  );
}

function Section({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle?: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <header className="mb-2 flex items-baseline justify-between">
        <h2 className="text-md font-semibold tracking-tight">{title}</h2>
        {subtitle && <span className="text-muted-foreground/70 text-xs">{subtitle}</span>}
      </header>
      <div className="border-border/60 bg-card/40 rounded-xl border p-4 backdrop-blur-xl md:p-5">
        {children}
      </div>
    </section>
  );
}
