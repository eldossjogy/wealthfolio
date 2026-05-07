import { useCallback, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { Link } from "react-router-dom";

import { useBalancePrivacy } from "@/hooks/use-balance-privacy";
import { QueryKeys } from "@/lib/query-keys";

import { AmountDisplay, Icons, Skeleton } from "@wealthfolio/ui";

import { getEventSpendingSummaries, listEventTypes } from "../../../adapters/events";
import { EventCategoryTreemap } from "../../event-category-treemap";
import { EventTimeline } from "../../event-timeline";
import type { ReportsRange } from "../../../lib/reports-period";
import type { EventSpendingSummary, EventType } from "../../../types/event";

interface EventsTabProps {
  range: ReportsRange;
  currency: string;
}

/**
 * Events tab — per-event analytics scoped to the active reports period.
 *
 * Reuses two existing presentational components from the events feature:
 * - `EventCategoryTreemap` — total category share across selected events
 * - `EventTimeline` — swim-lane timeline of events in the window
 */
export function EventsTab({ range, currency }: EventsTabProps) {
  const { isBalanceHidden } = useBalancePrivacy();
  const [selectedEventTypes, setSelectedEventTypes] = useState<Set<string>>(new Set());

  const { data: eventTypes = [] } = useQuery<EventType[]>({
    queryKey: [QueryKeys.SPENDING_EVENT_TYPES],
    queryFn: listEventTypes,
  });

  const requestPayload = useMemo(
    () => ({
      startDate: range.start.toISOString(),
      endDate: range.end.toISOString(),
    }),
    [range],
  );

  const { data: summaries = [], isLoading } = useQuery<EventSpendingSummary[]>({
    queryKey: [QueryKeys.SPENDING_EVENTS, "summaries", requestPayload],
    queryFn: () => getEventSpendingSummaries(requestPayload),
  });

  const toggleEventType = useCallback((eventTypeId: string) => {
    setSelectedEventTypes((prev) => {
      const next = new Set(prev);
      if (next.has(eventTypeId)) next.delete(eventTypeId);
      else next.add(eventTypeId);
      return next;
    });
  }, []);

  const filtered = useMemo(() => {
    if (selectedEventTypes.size === 0) return summaries;
    return summaries.filter((s) => selectedEventTypes.has(s.eventTypeId));
  }, [summaries, selectedEventTypes]);

  const totalSpending = useMemo(
    () => filtered.reduce((s, e) => s + e.totalSpending, 0),
    [filtered],
  );
  const totalTransactions = useMemo(
    () => filtered.reduce((s, e) => s + e.transactionCount, 0),
    [filtered],
  );
  const eventCurrency = filtered[0]?.currency ?? currency;

  if (isLoading) {
    return (
      <div className="space-y-6">
        <Skeleton className="h-24 w-full rounded-xl" />
        <Skeleton className="h-[260px] w-full rounded-xl" />
        <Skeleton className="h-[200px] w-full rounded-xl" />
      </div>
    );
  }

  if (summaries.length === 0) {
    return (
      <div className="border-border bg-card shadow-xs flex flex-col items-center justify-center rounded-xl border p-10 text-center">
        <Icons.Calendar className="text-muted-foreground mb-3 h-10 w-10" />
        <h3 className="mb-1 text-base font-semibold">No events in this window</h3>
        <p className="text-muted-foreground mb-3 text-sm">
          Trips, holidays, or special occasions tagged with events appear here.
        </p>
        <Link
          to="/settings/spending/events"
          className="text-primary inline-flex items-center gap-1 text-sm underline-offset-4 hover:underline"
        >
          Create an event
          <Icons.ChevronRight className="h-4 w-4" />
        </Link>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div className="grid gap-3 sm:grid-cols-3">
        <Tile label="Event spending" tone="default">
          <AmountDisplay
            value={totalSpending}
            currency={eventCurrency}
            isHidden={isBalanceHidden}
          />
        </Tile>
        <Tile label="Events" tone="default">
          <span className="tabular-nums">{filtered.length}</span>
        </Tile>
        <Tile label="Transactions" tone="default">
          <span className="tabular-nums">{totalTransactions}</span>
        </Tile>
      </div>

      <Section title="Categories across events" subtitle="Aggregate share of spend">
        <EventCategoryTreemap events={filtered} currency={eventCurrency} />
      </Section>

      <Section title="Event timeline" subtitle="Each event positioned on its date range">
        <EventTimeline
          events={summaries}
          eventTypes={eventTypes}
          selectedEventTypes={selectedEventTypes}
          onToggleEventType={toggleEventType}
          periodDateRange={{
            startDate: range.start.toISOString().slice(0, 10),
            endDate: range.end.toISOString().slice(0, 10),
          }}
        />
      </Section>
    </div>
  );
}

function Tile({ label, children }: { label: string; tone: "default"; children: React.ReactNode }) {
  return (
    <div className="border-border bg-card shadow-xs rounded-xl border p-4">
      <div className="text-muted-foreground/70 text-[11px] font-light uppercase tracking-wide">
        {label}
      </div>
      <div className="text-foreground mt-1 text-xl font-semibold tabular-nums">{children}</div>
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
      <div className="border-border bg-card shadow-xs rounded-xl border p-4 md:p-5">{children}</div>
    </section>
  );
}
