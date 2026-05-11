import { useCallback, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";

import { useTaxonomy } from "@/hooks/use-taxonomies";
import { QueryKeys } from "@/lib/query-keys";
import { useSettingsContext } from "@/lib/settings-provider";

import {
  AnimatedToggleGroup,
  Page,
  PageContent,
  PageHeader,
  usePersistentState,
} from "@wealthfolio/ui";

import { getEventSpendingSummaries } from "../adapters/events";
import { StageNav, type InsightsStage } from "../components/reports/insights/stage-nav";
import { WhatChangedStage } from "../components/reports/insights/what-changed-stage";
import { WhenWhereStage } from "../components/reports/insights/when-where-stage";
import { WhereIAmStage } from "../components/reports/insights/where-i-am-stage";
import { useBudget } from "../hooks/use-budget";
import { useCashActivities } from "../hooks/use-cash-activities";
import { useMonthlyHistory } from "../hooks/use-monthly-history";
import { useSpendingReport } from "../hooks/use-spending-report";
import {
  DEFAULT_REPORTS_PERIOD,
  REPORTS_PERIODS,
  comparisonRange,
  periodToReportsRange,
  rangeToReportRequest,
  type ReportsPeriod,
} from "../lib/reports-period";
import type { EventSpendingSummary } from "../types/event";

const SPENDING_TAXONOMY = "spending_categories";
const PERIOD_STORAGE_KEY = "spending-insights-period";
const STAGE_STORAGE_KEY = "spending-insights-stage";
const EMPTY_TAXONOMY: never[] = [];
/** Heatmap window — last 12 weeks regardless of selected period. */
const HEATMAP_WEEKS = 12;
const DAILY_GRANULARITY_THRESHOLD_DAYS = 35;

/**
 * Spending insights — narrative-first, three-stage page.
 *
 *   01 Where I am   — pace card + spent + cashflow + breakdown table
 *   02 What changed — period-vs-period headline + sparklines + delta table
 *   03 When & where — weekday-hour heatmap + events headline + per-event cards
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
  const { data: events = [] } = useQuery<EventSpendingSummary[]>({
    queryKey: [QueryKeys.SPENDING_EVENTS, "summaries", eventsRequest],
    queryFn: () => getEventSpendingSummaries(eventsRequest),
  });

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

  const useDailyForHistory = range.days <= DAILY_GRANULARITY_THRESHOLD_DAYS;
  const taxonomyCategories = taxonomy.data?.categories ?? EMPTY_TAXONOMY;

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
          // Prefer browser history; fall back to the spending hub for direct landings.
          if (window.history.length > 1) navigate(-1);
          else navigate("/spending");
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
          />
        )}

        {stage === "when" && (
          <WhenWhereStage
            heatmapActivities={heatmapActivities}
            events={events}
            taxonomyCategories={taxonomyCategories}
            currency={baseCurrency}
            rangeStart={range.start}
            rangeEnd={range.end}
          />
        )}
      </PageContent>
    </Page>
  );
}
