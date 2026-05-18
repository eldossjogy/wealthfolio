import { useEffect, useMemo, useRef, useState, type FC } from "react";
import { Link } from "react-router-dom";

import { Button, Icons, formatCompactAmount } from "@wealthfolio/ui";
import type { Activity, TaxonomyCategory } from "@/lib/types";
import { cn, formatAmount } from "@/lib/utils";

import { useSpendingEventMutations } from "../../../hooks/use-spending-events";
import { getActivitySpendingAmount } from "../../../lib/constants";
import type { EventSpendingSummary } from "../../../types/event";
import { formatMonthDay } from "./format";
import { WhenYouSpendCard } from "./when-you-spend-card";

const CARD_CLASS = "border-border/60 bg-card/40 rounded-2xl border p-5 backdrop-blur-xl";
const LABEL_CLASS = "text-muted-foreground/70 text-[10px] font-normal uppercase tracking-[0.12em]";

export interface WhenWhereStageProps {
  /** Last 12 weeks of cash activities (for the heatmap). */
  heatmapActivities: Activity[];
  accountTypeById?: Map<string, string>;
  events: EventSpendingSummary[];
  taxonomyCategories: TaxonomyCategory[];
  currency: string;
  /** Period start/end for the events strip. */
  rangeStart: Date;
  rangeEnd: Date;
  /** Fired when a heatmap cell is clicked. Weekday is Mon=0..Sun=6, hour is 0..23. */
  onHeatmapCellClick?: (weekday: number, hour: number) => void;
}

export function WhenWhereStage({
  heatmapActivities,
  accountTypeById,
  events,
  taxonomyCategories,
  currency,
  rangeStart,
  rangeEnd,
  onHeatmapCellClick,
}: WhenWhereStageProps) {
  // Derived: user's pick wins if it's still in the list; otherwise fall back to
  // the first event. Avoids the prop-mirror useEffect pattern.
  const [override, setOverride] = useState<string | null>(null);
  const selectedId =
    override && events.some((e) => e.eventId === override)
      ? override
      : (events[0]?.eventId ?? null);
  const selected = useMemo(
    () => events.find((e) => e.eventId === selectedId) ?? null,
    [events, selectedId],
  );

  return (
    <div className="flex flex-col gap-6">
      <WhenYouSpendCard
        activities={heatmapActivities}
        accountTypeById={accountTypeById}
        currency={currency}
        onCellClick={onHeatmapCellClick}
      />
      <div className="flex flex-col gap-4">
        {events.length > 0 ? (
          <>
            <EventsTimelineCard
              events={events}
              currency={currency}
              rangeStart={rangeStart}
              rangeEnd={rangeEnd}
              heatmapActivities={heatmapActivities}
              accountTypeById={accountTypeById}
              selectedId={selectedId}
              onSelect={setOverride}
            />
            {selected && (
              <EventDetailPanel
                event={selected}
                events={events}
                taxonomyCategories={taxonomyCategories}
                currency={currency}
                heatmapActivities={heatmapActivities}
                accountTypeById={accountTypeById}
                onSelect={setOverride}
              />
            )}
          </>
        ) : (
          <EmptyEventsCard />
        )}
      </div>
    </div>
  );
}
// ═════════════════════════════════════════════════════════════════════════
// Events headline + timeline strip
// ═════════════════════════════════════════════════════════════════════════

interface EventsTimelineCardProps {
  events: EventSpendingSummary[];
  currency: string;
  rangeStart: Date;
  rangeEnd: Date;
  /** Last 12 weeks of cash activities; used for daily series + normal pace. */
  heatmapActivities: Activity[];
  accountTypeById?: Map<string, string>;
  selectedId: string | null;
  onSelect: (id: string) => void;
}

const MONTH_LABELS = [
  "JAN",
  "FEB",
  "MAR",
  "APR",
  "MAY",
  "JUN",
  "JUL",
  "AUG",
  "SEP",
  "OCT",
  "NOV",
  "DEC",
];

// Kind palette — fallback only. In practice the user picks a color when
// creating an event type, so `getEventColors` returns the user color and this
// table is only hit for legacy/imported events with no color set.
const KIND_COLORS = {
  trip: { stroke: "#4F6B92", fill: "#D8E1EE" },
  wedding: { stroke: "#B0552E", fill: "#EFD2C2" },
  holiday: { stroke: "#6B8E54", fill: "#D4DEC7" },
  move: { stroke: "#B89A4C", fill: "#EBDDB7" },
  oneoff: { stroke: "#8E7CB3", fill: "#DCD3EA" },
} as const;

type EventKind = keyof typeof KIND_COLORS;

function inferEventKind(typeName: string | null | undefined): EventKind {
  const n = (typeName ?? "").toLowerCase();
  if (n.includes("trip") || n.includes("travel") || n.includes("flight") || n.includes("vacation"))
    return "trip";
  if (n.includes("wedding")) return "wedding";
  if (n.includes("holiday")) return "holiday";
  if (n.includes("move") || n.includes("apartment") || n.includes("home")) return "move";
  return "oneoff";
}

/** Resolve stroke/fill for an event. Prefers eventTypeColor; otherwise uses the kind palette. */
function getEventColors(ev: EventSpendingSummary): { stroke: string; fill: string } {
  const kind = inferEventKind(ev.eventTypeName);
  if (ev.eventTypeColor) {
    // Use custom stroke; derive a soft fill by appending alpha hex.
    return { stroke: ev.eventTypeColor, fill: `${ev.eventTypeColor}33` };
  }
  return KIND_COLORS[kind];
}

const EventsTimelineCard: FC<EventsTimelineCardProps> = ({
  events,
  currency,
  rangeStart,
  rangeEnd,
  heatmapActivities,
  accountTypeById,
  selectedId,
  onSelect,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);

  useEffect(() => {
    if (!containerRef.current) return;
    const node = containerRef.current;
    const update = () => setWidth(node.clientWidth);
    update();
    const ro = new ResizeObserver(update);
    ro.observe(node);
    return () => ro.disconnect();
  }, []);

  const computed = useMemo(
    () => computeEventsAggregate(events, heatmapActivities, accountTypeById),
    [accountTypeById, events, heatmapActivities],
  );

  const dailySeries = useMemo(
    () => buildDailySeries(heatmapActivities, events, accountTypeById, rangeStart, rangeEnd),
    [heatmapActivities, events, accountTypeById, rangeStart, rangeEnd],
  );

  const periodDays = Math.max(1, inclusiveDays(rangeStart, rangeEnd));
  const W = Math.max(640, width || 1232);
  const padL = 14;
  const padR = 64;
  const innerW = W - padL - padR;
  const dayW = innerW / periodDays;

  const bandsTop = 48;
  const bandsH = 56;
  const LANE_STRIDE = bandsH + 6;

  // Month markers
  const months = useMemo(() => buildMonthMarkers(rangeStart, rangeEnd), [rangeStart, rangeEnd]);

  // Narrow-event label stacking — adjacent narrow bands stagger label rows.
  const WIDE_THRESHOLD = 50;
  const NARROW_LABEL_W = 110;

  // Wide-band lane assignment — overlapping wide bands stack vertically so
  // labels don't collide. Sorted by start x; each band claims the lowest lane
  // whose previous occupant ended before this one starts.
  const wideLaneByEventId = useMemo(() => {
    const result: Record<string, number> = {};
    const wide = events
      .map((e) => {
        const start = new Date(e.startDate);
        const end = new Date(e.endDate);
        const a = Math.max(0, Math.round((start.getTime() - rangeStart.getTime()) / 86_400_000));
        const b = Math.min(
          periodDays - 1,
          Math.round((end.getTime() - rangeStart.getTime()) / 86_400_000),
        );
        const x1 = padL + a * dayW;
        const w = Math.max((b - a + 1) * dayW, 6);
        return { id: e.eventId, x1, x2: x1 + w, wide: w > WIDE_THRESHOLD };
      })
      .filter((it) => it.wide)
      .sort((a, b) => a.x1 - b.x1);
    const laneRights: number[] = [];
    for (const item of wide) {
      let lane = 0;
      while (laneRights[lane] != null && laneRights[lane] > item.x1) lane++;
      laneRights[lane] = item.x2;
      result[item.id] = lane;
    }
    return result;
  }, [events, rangeStart, periodDays, dayW]);

  const wideLaneCount = Math.max(1, ...Object.values(wideLaneByEventId).map((l) => l + 1));
  const bandsAreaH = bandsH + (wideLaneCount - 1) * LANE_STRIDE;
  const chartTop = bandsTop + bandsAreaH + 18;
  const chartH = 96;
  const axisTop = chartTop + chartH;
  const totalH = axisTop + 30;

  // Memoize SVG geometry — rebuilds only when daily series or chart dims change,
  // not on every parent render (selectedId change, etc).
  const { linePath, areaPath, yNormal, todayX, showToday } = useMemo(() => {
    const maxDaily = Math.max(1, ...dailySeries);
    const scaleMax = Math.max(maxDaily * 1.1, computed.normalPace * 2.2);
    const yDaily = (v: number) => chartTop + chartH - (Math.min(v, scaleMax) / scaleMax) * chartH;
    const points = dailySeries.map((v, i) => [padL + (i + 0.5) * dayW, yDaily(v)] as const);
    const linePath = points.map(([x, y], i) => (i === 0 ? `M${x},${y}` : `L${x},${y}`)).join(" ");
    const areaPath = `${linePath} L${padL + innerW},${chartTop + chartH} L${padL},${chartTop + chartH} Z`;
    const today = new Date();
    const todayIdx = Math.round((today.getTime() - rangeStart.getTime()) / 86_400_000);
    return {
      linePath,
      areaPath,
      yNormal: yDaily(computed.normalPace),
      todayX: padL + (todayIdx + 0.5) * dayW,
      showToday: todayIdx >= 0 && todayIdx <= periodDays - 1,
    };
  }, [
    dailySeries,
    computed.normalPace,
    chartTop,
    chartH,
    padL,
    dayW,
    innerW,
    rangeStart,
    periodDays,
  ]);

  const labelRowByEventId = useMemo(() => {
    const result: Record<string, number> = {};
    const rowEnds: number[] = [];
    const indexed = events.map((e) => {
      const start = new Date(e.startDate);
      const end = new Date(e.endDate);
      const a = Math.max(0, Math.round((start.getTime() - rangeStart.getTime()) / 86_400_000));
      const b = Math.min(
        periodDays - 1,
        Math.round((end.getTime() - rangeStart.getTime()) / 86_400_000),
      );
      return { e, a, b, x: padL + a * dayW };
    });
    indexed.sort((a, b) => a.x - b.x);
    for (const { e, a, b, x } of indexed) {
      const w = Math.max((b - a + 1) * dayW, 6);
      if (w > WIDE_THRESHOLD) continue;
      const labelStart = x + w / 2 - NARROW_LABEL_W / 2;
      const labelEnd = labelStart + NARROW_LABEL_W;
      let row = 0;
      while (rowEnds[row] != null && rowEnds[row] > labelStart) row++;
      rowEnds[row] = labelEnd;
      result[e.eventId] = row;
    }
    return result;
  }, [events, rangeStart, periodDays, dayW]);

  const selected = events.find((e) => e.eventId === selectedId) ?? events[events.length - 1];
  const biggest = useMemo(
    () => events.slice().sort((a, b) => b.totalSpending - a.totalSpending)[0],
    [events],
  );

  // Legend mirrors the actual event types present in the data — same color
  // source as the bands so the swatches always match what's drawn.
  const usedTypes = useMemo(() => {
    const map = new Map<string, { id: string; name: string; stroke: string; fill: string }>();
    for (const ev of events) {
      if (map.has(ev.eventTypeId)) continue;
      const c = getEventColors(ev);
      map.set(ev.eventTypeId, {
        id: ev.eventTypeId,
        name: ev.eventTypeName ?? "Event",
        stroke: c.stroke,
        fill: c.fill,
      });
    }
    return Array.from(map.values());
  }, [events]);

  return (
    <div className={cn(CARD_CLASS, "font-mono")} ref={containerRef}>
      {/* HEADER */}
      <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
        <div>
          <div className="text-foreground text-base font-semibold tracking-tight">Events</div>
          <div className="text-muted-foreground/80 mt-0.5 text-[11px]">
            {events.length} tagged event{events.length === 1 ? "" : "s"} across{" "}
            {computed.totalEventDays} days · click any band to inspect
          </div>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1.5">
          {usedTypes.map((t) => (
            <span
              key={t.id}
              className="text-muted-foreground/80 inline-flex items-center gap-1.5 text-[10px] tracking-wider"
            >
              <span
                className="inline-block h-2.5 w-2.5 rounded-[2px]"
                style={{ background: t.fill, border: `1.5px solid ${t.stroke}` }}
              />
              {t.name.toUpperCase()}
            </span>
          ))}
          <Button asChild variant="outline" size="sm" className="ml-1 h-7 text-[11px]">
            <Link to="/settings/spending/events">+ TAG EVENT</Link>
          </Button>
        </div>
      </div>

      {/* TIMELINE CHART */}
      <div className="relative w-full">
        <svg width={W} height={totalH} style={{ display: "block", overflow: "visible" }}>
          {/* Month gridlines + labels */}
          {months.map((m, i) => {
            const x = padL + m.idx * dayW;
            const showYear = m.label === "JAN" || i === 0;
            return (
              <g key={i}>
                <line
                  x1={x}
                  x2={x}
                  y1={20}
                  y2={chartTop + chartH}
                  stroke="currentColor"
                  className="text-foreground/10"
                  strokeDasharray="2 3"
                />
                <text
                  x={x + 6}
                  y={14}
                  className="fill-muted-foreground/80"
                  fontSize={10}
                  letterSpacing={0.5}
                >
                  {m.label}
                  {showYear ? ` ${m.year}` : ""}
                </text>
              </g>
            );
          })}

          {/* Normal-pace baseline */}
          {computed.normalPace > 0 && (
            <>
              <line
                x1={padL}
                x2={padL + innerW}
                y1={yNormal}
                y2={yNormal}
                stroke="currentColor"
                className="text-muted-foreground/60"
                strokeDasharray="3 3"
              />
              <text
                x={padL + innerW + 4}
                y={yNormal + 3}
                fontSize={9}
                className="fill-muted-foreground"
              >
                {formatCompactAmount(computed.normalPace, currency)}/d
              </text>
            </>
          )}

          {/* Daily area */}
          <path d={areaPath} fill="currentColor" className="text-foreground/5" />
          <path
            d={linePath}
            fill="none"
            stroke="currentColor"
            strokeWidth={1}
            className="text-muted-foreground/60"
          />

          {/* Highlight event regions on the daily chart */}
          {events.map((ev) => {
            const start = new Date(ev.startDate);
            const end = new Date(ev.endDate);
            const a = Math.max(
              0,
              Math.round((start.getTime() - rangeStart.getTime()) / 86_400_000),
            );
            const b = Math.min(
              periodDays - 1,
              Math.round((end.getTime() - rangeStart.getTime()) / 86_400_000),
            );
            if (b < 0 || a > periodDays - 1) return null;
            const x1 = padL + a * dayW;
            const x2 = padL + (b + 1) * dayW;
            const c = getEventColors(ev);
            const isSel = selectedId === ev.eventId;
            return (
              <rect
                key={"hl-" + ev.eventId}
                x={x1}
                y={chartTop - 2}
                width={x2 - x1}
                height={chartH + 4}
                fill={c.fill}
                opacity={isSel ? 0.55 : 0.28}
              />
            );
          })}

          {/* Re-stroke chart line on top of highlights */}
          <path
            d={linePath}
            fill="none"
            stroke="currentColor"
            strokeWidth={1.2}
            className="text-foreground/80"
          />

          {/* Event bands */}
          {events.map((ev) => {
            const start = new Date(ev.startDate);
            const end = new Date(ev.endDate);
            const a = Math.max(
              0,
              Math.round((start.getTime() - rangeStart.getTime()) / 86_400_000),
            );
            const b = Math.min(
              periodDays - 1,
              Math.round((end.getTime() - rangeStart.getTime()) / 86_400_000),
            );
            if (b < 0 || a > periodDays - 1) return null;
            const x1 = padL + a * dayW;
            const x2 = padL + (b + 1) * dayW;
            const w = Math.max(x2 - x1, 6);
            const isSel = selectedId === ev.eventId;
            const c = getEventColors(ev);
            const days = Math.max(1, inclusiveDays(start, end));
            const expected = computed.normalPace * days;
            const lift = ev.totalSpending - expected;
            const kindLabel = (ev.eventTypeName ?? "").toUpperCase();
            const labelRowIdx = labelRowByEventId[ev.eventId] ?? 0;
            const labelYOffset = -4 - labelRowIdx * 12;
            const isWide = w > WIDE_THRESHOLD;
            const bandY = isWide
              ? bandsTop + (wideLaneByEventId[ev.eventId] ?? 0) * LANE_STRIDE
              : bandsTop;

            return (
              <g
                key={ev.eventId}
                style={{ cursor: "pointer" }}
                onClick={() => onSelect(ev.eventId)}
              >
                <rect
                  x={x1}
                  y={bandY}
                  width={w}
                  height={bandsH - 4}
                  fill={c.fill}
                  stroke={c.stroke}
                  strokeWidth={isSel ? 2 : 1}
                  rx={4}
                  opacity={isSel ? 1 : 0.85}
                />
                <rect
                  x={x1}
                  y={bandY}
                  width={3}
                  height={bandsH - 4}
                  fill={c.stroke}
                  opacity={isSel ? 1 : 0.7}
                />

                {isWide ? (
                  <>
                    <text
                      x={x1 + 8}
                      y={bandY + 16}
                      fontSize={11}
                      className="fill-foreground"
                      fontWeight={isSel ? 700 : 600}
                    >
                      {ev.eventName}
                    </text>
                    <text
                      x={x1 + 8}
                      y={bandY + 32}
                      fontSize={9.5}
                      fontWeight={600}
                      className={lift >= 0 ? "fill-destructive" : "fill-success"}
                    >
                      {lift >= 0 ? "+" : "−"}
                      {formatCompactAmount(Math.abs(lift), currency)}
                    </text>
                    <text x={x1 + 8} y={bandY + 46} fontSize={9} className="fill-muted-foreground">
                      {days}D · {kindLabel}
                    </text>
                  </>
                ) : (
                  <g>
                    {labelRowIdx > 0 && (
                      <line
                        x1={x1 + w / 2}
                        x2={x1 + w / 2}
                        y1={bandY}
                        y2={bandY + labelYOffset + 2}
                        stroke={c.stroke}
                        strokeWidth={1}
                        opacity={0.5}
                      />
                    )}
                    <text
                      x={x1 + w / 2}
                      y={bandY + labelYOffset}
                      fontSize={10}
                      fontWeight={isSel ? 700 : 500}
                      textAnchor="middle"
                      className={isSel ? "fill-foreground" : "fill-foreground/80"}
                    >
                      {ev.eventName}
                    </text>
                  </g>
                )}
              </g>
            );
          })}

          {/* Today marker */}
          {showToday && (
            <>
              <line
                x1={todayX}
                x2={todayX}
                y1={4}
                y2={chartTop + chartH}
                stroke="var(--event-today)"
                strokeWidth={1.5}
              />
              <circle cx={todayX} cy={4} r={3} fill="var(--event-today)" />
              <text x={todayX + 6} y={14} fontSize={9.5} fontWeight={600} fill="var(--event-today)">
                TODAY
              </text>
            </>
          )}

          {/* Bookend dates */}
          <text x={padL} y={axisTop + 14} fontSize={9.5} className="fill-muted-foreground">
            {formatBookendDate(rangeStart)}
          </text>
          <text
            x={padL + innerW}
            y={axisTop + 14}
            fontSize={9.5}
            textAnchor="end"
            className="fill-muted-foreground"
          >
            {formatBookendDate(rangeEnd)} · {periodDays} DAYS
          </text>
          <text
            x={padL + innerW / 2}
            y={axisTop + 14}
            fontSize={9.5}
            textAnchor="middle"
            className="fill-muted-foreground/70"
          >
            DAILY SPEND
          </text>
        </svg>
      </div>

      {/* Summary strip */}
      <div className="border-border/40 mt-4 grid grid-cols-2 gap-x-0 gap-y-4 border-t pt-4 md:grid-cols-4">
        <SummaryCell label={`ACROSS ${events.length} EVENT${events.length === 1 ? "" : "S"}`}>
          <div className="text-foreground text-lg font-semibold tabular-nums tracking-tight">
            {formatAmount(computed.totalSpent, currency)}
          </div>
          <div className="text-muted-foreground/80 mt-0.5 text-[10px]">
            {computed.totalEventDays} event-days ·{" "}
            {Math.round((computed.totalEventDays / periodDays) * 100)}% of period
          </div>
        </SummaryCell>
        <SummaryCell label="COMBINED LIFT" divided>
          <div
            className={cn(
              "text-lg font-semibold tabular-nums tracking-tight",
              computed.lift >= 0 ? "text-destructive" : "text-success",
            )}
          >
            {computed.lift >= 0 ? "+" : "−"}
            {formatAmount(Math.abs(computed.lift), currency)}
          </div>
          <div className="text-muted-foreground/80 mt-0.5 text-[10px]">
            on event days, vs normal pace
          </div>
        </SummaryCell>
        {biggest && (
          <SummaryCell label="BIGGEST EVENT" divided>
            <div className="text-foreground truncate text-lg font-semibold tracking-tight">
              {biggest.eventName}
            </div>
            <div className="text-muted-foreground/80 mt-0.5 text-[10px] tabular-nums">
              {formatAmount(biggest.totalSpending, currency)}
            </div>
          </SummaryCell>
        )}
        {selected && (
          <SummaryCell label="SELECTED" divided>
            <div className="mt-0.5 inline-flex items-center gap-2">
              <span
                className="inline-block h-2 w-2 shrink-0 rounded-[2px]"
                style={{
                  background: getEventColors(selected).fill,
                  border: `1.5px solid ${getEventColors(selected).stroke}`,
                }}
              />
              <span className="text-foreground truncate text-lg font-semibold tracking-tight">
                {selected.eventName}
              </span>
            </div>
            <div className="text-muted-foreground/80 mt-0.5 text-[10px] tabular-nums">
              {formatSelectedRange(new Date(selected.startDate), new Date(selected.endDate))} ·{" "}
              {inclusiveDays(new Date(selected.startDate), new Date(selected.endDate))}D
            </div>
          </SummaryCell>
        )}
      </div>
    </div>
  );
};

function SummaryCell({
  label,
  divided,
  children,
}: {
  label: string;
  divided?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className={cn(divided && "md:border-border/40 md:border-l md:pl-4")}>
      <div className={LABEL_CLASS}>{label}</div>
      <div className="mt-1">{children}</div>
    </div>
  );
}

interface EventsAggregate {
  totalSpent: number;
  totalEventDays: number;
  normalPace: number;
  lift: number;
  topEventName: string | null;
}

function computeEventsAggregate(
  events: EventSpendingSummary[],
  heatmapActivities: Activity[],
  accountTypeById?: Map<string, string>,
): EventsAggregate {
  let totalSpent = 0;
  let totalEventDays = 0;
  let topEvent: EventSpendingSummary | null = null;
  for (const ev of events) {
    totalSpent += ev.totalSpending;
    const days = Math.max(1, inclusiveDays(new Date(ev.startDate), new Date(ev.endDate)));
    totalEventDays += days;
    if (!topEvent || ev.totalSpending > topEvent.totalSpending) topEvent = ev;
  }

  // Normal pace = average daily outflow over the heatmap activity window
  // (last 12 weeks), excluding event days.
  const eventDayKeys = new Set<string>();
  for (const ev of events) {
    const start = new Date(ev.startDate);
    const end = new Date(ev.endDate);
    for (let d = new Date(start); d <= end; d.setDate(d.getDate() + 1)) {
      eventDayKeys.add(d.toISOString().slice(0, 10));
    }
  }
  let baselineTotal = 0;
  let baselineDays = 0;
  const baselineDaySeen = new Set<string>();
  for (const a of heatmapActivities) {
    const spendingAmount = getActivitySpendingAmount(a, accountTypeById?.get(a.accountId));
    if (spendingAmount === 0) continue;
    const dayKey = new Date(a.activityDate).toISOString().slice(0, 10);
    if (eventDayKeys.has(dayKey)) continue;
    baselineTotal += spendingAmount;
    baselineDaySeen.add(dayKey);
  }
  baselineDays = Math.max(1, baselineDaySeen.size);
  const normalPace = Math.max(0, baselineTotal) / baselineDays;
  const expected = normalPace * totalEventDays;
  const lift = totalSpent - expected;

  return {
    totalSpent,
    totalEventDays,
    normalPace,
    lift,
    topEventName: topEvent?.eventName ?? null,
  };
}

/** Inclusive day count between A and B — same day = 1, next day = 2, etc. */
function inclusiveDays(a: Date, b: Date): number {
  return Math.max(0, Math.round((b.getTime() - a.getTime()) / (24 * 3600 * 1000))) + 1;
}

/** Build a per-day spend series across [rangeStart, rangeEnd]. */
function buildDailySeries(
  activities: Activity[],
  events: EventSpendingSummary[],
  accountTypeById: Map<string, string> | undefined,
  rangeStart: Date,
  rangeEnd: Date,
): number[] {
  const periodDays = Math.max(1, inclusiveDays(rangeStart, rangeEnd));
  const series = new Array(periodDays).fill(0);
  const startMs = rangeStart.getTime();

  for (const a of activities) {
    const amt = getActivitySpendingAmount(a, accountTypeById?.get(a.accountId));
    if (amt <= 0) continue;
    const idx = Math.round((new Date(a.activityDate).getTime() - startMs) / 86_400_000);
    if (idx >= 0 && idx < periodDays) series[idx] += amt;
  }

  // Overlay event-level dailySpending (covers periods outside the 12-week window).
  for (const ev of events) {
    for (const [dateKey, amount] of Object.entries(ev.dailySpending ?? {})) {
      const day = new Date(`${dateKey}T12:00:00`);
      const idx = Math.round((day.getTime() - startMs) / 86_400_000);
      if (idx >= 0 && idx < periodDays && amount > 0) series[idx] = amount;
    }
  }
  return series;
}

interface MonthMarker {
  idx: number;
  label: string;
  year: number;
}

function buildMonthMarkers(rangeStart: Date, rangeEnd: Date): MonthMarker[] {
  const out: MonthMarker[] = [];
  const cursor = new Date(rangeStart.getFullYear(), rangeStart.getMonth(), 1, 12, 0, 0, 0);
  while (cursor <= rangeEnd) {
    const idx = Math.round((cursor.getTime() - rangeStart.getTime()) / 86_400_000);
    out.push({ idx, label: MONTH_LABELS[cursor.getMonth()], year: cursor.getFullYear() });
    cursor.setMonth(cursor.getMonth() + 1);
  }
  return out;
}

function formatBookendDate(d: Date): string {
  return `${MONTH_LABELS[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()}`;
}

function formatSelectedRange(start: Date, end: Date): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${pad(start.getMonth() + 1)}/${pad(start.getDate())} – ${pad(end.getMonth() + 1)}/${pad(end.getDate())}`;
}

// ═════════════════════════════════════════════════════════════════════════
// Rich event detail panel — replaces the old 3-up EventDetailCard grid
// ═════════════════════════════════════════════════════════════════════════

interface EventDetailPanelProps {
  event: EventSpendingSummary;
  events: EventSpendingSummary[];
  taxonomyCategories: TaxonomyCategory[];
  currency: string;
  heatmapActivities: Activity[];
  accountTypeById?: Map<string, string>;
  onSelect: (id: string) => void;
}

const EventDetailPanel: FC<EventDetailPanelProps> = ({
  event,
  events,
  taxonomyCategories,
  currency,
  heatmapActivities,
  accountTypeById,
  onSelect,
}) => {
  const startDate = useMemo(() => new Date(event.startDate), [event.startDate]);
  const endDate = useMemo(() => new Date(event.endDate), [event.endDate]);
  const days = Math.max(1, inclusiveDays(startDate, endDate));
  const dailyDuring = days > 0 ? event.totalSpending / days : 0;

  const baseline = useMemo(
    () => computeBaselinePace(heatmapActivities, [event], accountTypeById),
    [accountTypeById, heatmapActivities, event],
  );

  const expected = baseline * days;
  const lift = event.totalSpending - expected;
  const dailyDeltaPct = baseline > 0 ? Math.round((dailyDuring / baseline - 1) * 100) : 0;

  const categories = useMemo(
    () => buildEventCategoryRows(event, taxonomyCategories),
    [event, taxonomyCategories],
  );
  const categoriesTotal = categories.reduce((sum, c) => sum + c.amount, 0);

  const dailySeries = useMemo(() => buildEventDailySeries(event, days), [event, days]);
  const peak = useMemo(() => findPeakDay(event, dailySeries), [event, dailySeries]);

  const beforeSeries = useMemo(
    () => buildWindowSeries(heatmapActivities, accountTypeById, startDate, -7, 7),
    [heatmapActivities, accountTypeById, startDate],
  );
  const afterSeries = useMemo(
    () => buildWindowSeries(heatmapActivities, accountTypeById, endDate, 1, 3),
    [heatmapActivities, accountTypeById, endDate],
  );
  const beforeAvg = avgSeries(beforeSeries);
  const afterAvg = avgSeries(afterSeries);
  const hangoverPct = baseline > 0 ? Math.round((afterAvg / baseline - 1) * 100) : 0;

  const tagColor = event.eventTypeColor ?? "var(--event-default)";

  const currentIdx = events.findIndex((e) => e.eventId === event.eventId);
  const canNav = events.length > 1;
  const prevEvent = canNav ? events[(currentIdx - 1 + events.length) % events.length] : null;
  const nextEvent = canNav ? events[(currentIdx + 1) % events.length] : null;

  const caption = useMemo(
    () => buildEventCaption({ days, lift, currency, top: categories }),
    [days, lift, currency, categories],
  );

  // Detect tagged transactions falling outside the event's date window. The
  // backend's `dailySpending` covers every day with tagged spend, including
  // dates outside the event's own [start, end].
  const outOfRange = useMemo(() => {
    const s = event.startDate.slice(0, 10);
    const e = event.endDate.slice(0, 10);
    const dates: string[] = [];
    for (const dateKey of Object.keys(event.dailySpending ?? {})) {
      const k = dateKey.slice(0, 10);
      if (k < s || k > e) dates.push(k);
    }
    dates.sort();
    return dates;
  }, [event.dailySpending, event.startDate, event.endDate]);

  const { update } = useSpendingEventMutations();
  const expandWindow = () => {
    if (outOfRange.length === 0) return;
    const all = [...outOfRange, event.startDate.slice(0, 10), event.endDate.slice(0, 10)].sort();
    update.mutate({
      id: event.eventId,
      patch: { startDate: all[0], endDate: all[all.length - 1] },
    });
  };

  return (
    <div className={cn(CARD_CLASS, "font-mono")}>
      {/* HEADER */}
      <div className="mb-3 flex flex-wrap items-end justify-between gap-3">
        <div className="flex min-w-0 items-center gap-2.5">
          <span
            className="inline-block h-2.5 w-2.5 shrink-0 rounded-[2px]"
            style={{ background: `${tagColor}26`, border: `1.5px solid ${tagColor}` }}
          />
          <div className="min-w-0">
            <div className="text-foreground truncate text-base font-semibold tracking-tight">
              {event.eventName}
            </div>
            <div className="text-muted-foreground/80 mt-0.5 text-[11px]">
              {formatRange(startDate, endDate)} · {days} day{days === 1 ? "" : "s"} ·{" "}
              {event.transactionCount} tx
              {event.eventTypeName ? ` · ${event.eventTypeName.toLowerCase()}` : ""}
            </div>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1.5">
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-[11px]"
            onClick={() => prevEvent && onSelect(prevEvent.eventId)}
            disabled={!canNav}
          >
            ← PREV
          </Button>
          <Button
            variant="outline"
            size="sm"
            className="h-7 text-[11px]"
            onClick={() => nextEvent && onSelect(nextEvent.eventId)}
            disabled={!canNav}
          >
            NEXT →
          </Button>
        </div>
      </div>

      {outOfRange.length > 0 && (
        <div className="bg-warning/10 border-warning/40 mt-3 flex flex-wrap items-center justify-between gap-2 rounded-md border px-2.5 py-1.5 text-[11px]">
          <span className="text-foreground/90">
            <span className="font-medium tabular-nums">{outOfRange.length}</span> tagged tx outside
            event dates
            <span className="text-muted-foreground/80 ml-1 tabular-nums">
              ({formatOutOfRangeDate(outOfRange[0])}
              {outOfRange.length > 1
                ? `–${formatOutOfRangeDate(outOfRange[outOfRange.length - 1])}`
                : ""}
              )
            </span>
          </span>
          <button
            type="button"
            onClick={expandWindow}
            disabled={update.isPending}
            className="text-foreground hover:bg-warning/15 rounded px-2 py-0.5 text-[11px] font-medium underline-offset-2 hover:underline disabled:opacity-50"
          >
            {update.isPending ? "Expanding…" : "Expand window →"}
          </button>
        </div>
      )}

      {/* STAT BLOCK */}
      <div className="mt-2 grid grid-cols-2 gap-y-4 md:grid-cols-4 md:gap-x-0">
        <StatCell label="EVENT TOTAL">
          <div className="text-foreground text-base font-semibold tabular-nums tracking-tight">
            {formatAmount(event.totalSpending, currency)}
          </div>
          <div className="text-muted-foreground/80 mt-1 text-[10px]">
            across {event.transactionCount} transactions
          </div>
        </StatCell>
        <StatCell label="LIFT VS NORMAL" divided>
          <div
            className={cn(
              "text-base font-semibold tabular-nums tracking-tight",
              lift >= 0 ? "text-destructive" : "text-success",
            )}
          >
            {lift >= 0 ? "+" : "−"}
            {formatAmount(Math.abs(lift), currency)}
          </div>
          <div className="text-muted-foreground/80 mt-1 text-[10px]">
            vs {formatAmount(Math.max(0, expected), currency)} expected
          </div>
        </StatCell>
        <StatCell label="DAILY DURING" divided>
          <div className="text-foreground text-base font-semibold tabular-nums tracking-tight">
            {formatAmount(dailyDuring, currency)}
          </div>
          <div className="text-muted-foreground/80 mt-1 text-[10px]">
            {baseline > 0
              ? `${dailyDeltaPct >= 0 ? "+" : "−"}${Math.abs(dailyDeltaPct)}% vs ${formatAmount(baseline, currency)}`
              : "no baseline available"}
          </div>
        </StatCell>
        <StatCell label="PEAK DAY" divided>
          <div className="text-foreground text-base font-semibold tabular-nums tracking-tight">
            {peak ? formatAmount(peak.amount, currency) : "—"}
          </div>
          <div className="text-muted-foreground/80 mt-1 text-[10px]">
            {peak ? formatPeakDay(peak.date) : ""}
          </div>
        </StatCell>
      </div>

      {/* TAKEAWAY */}
      <p className="text-foreground/90 mt-6 text-[13px] leading-relaxed">
        <span className={cn(LABEL_CLASS, "mr-2")}>TAKEAWAY</span>
        {caption}
      </p>

      {/* DAY BY DAY · WHAT DROVE IT */}
      <div className="mt-6 grid grid-cols-1 gap-x-8 gap-y-6 md:grid-cols-2">
        {/* LEFT: DAY BY DAY */}
        <div>
          <div className="flex items-center justify-between gap-3">
            <div className={LABEL_CLASS}>DAY BY DAY</div>
            <div className={cn(LABEL_CLASS, "text-right")}>
              {peak
                ? `PEAK ${formatAmount(peak.amount, currency)} · BASELINE ${formatAmount(baseline, currency)}`
                : `BASELINE ${formatAmount(baseline, currency)}`}
            </div>
          </div>
          <DailyBars
            beforeSeries={beforeSeries}
            duringSeries={dailySeries}
            afterSeries={afterSeries}
            startDate={startDate}
            endDate={endDate}
            baseline={baseline}
            currency={currency}
          />
        </div>

        {/* RIGHT: WHAT DROVE IT */}
        <div>
          <div className="flex items-center justify-between gap-3">
            <div className={LABEL_CLASS}>WHAT DROVE IT</div>
            <div className={cn(LABEL_CLASS, "text-right")}>
              {categories.length} CATEGOR{categories.length === 1 ? "Y" : "IES"}
            </div>
          </div>
          {categories.length > 0 && (
            <>
              <div className="mt-3 flex h-1.5 items-stretch gap-0.5">
                {categories.map((c) => (
                  <div
                    key={c.id}
                    className="rounded-full"
                    title={`${c.name} · ${formatAmount(c.amount, currency)}`}
                    style={{ flex: `${c.amount} 0 0`, background: c.color }}
                  />
                ))}
              </div>
              <div className="mt-2">
                {categories.map((c) => {
                  const pct =
                    categoriesTotal > 0 ? Math.round((c.amount / categoriesTotal) * 1000) / 10 : 0;
                  return (
                    <div
                      key={c.id}
                      className="border-border/30 flex items-center gap-3 border-b py-1.5 last:border-b-0"
                    >
                      <span
                        className="inline-block h-2 w-2 shrink-0 rounded-full"
                        style={{ background: c.color }}
                      />
                      <span className="text-foreground/90 min-w-0 flex-1 truncate text-[12px]">
                        {c.name}
                      </span>
                      <span className="text-muted-foreground/80 text-[11px] tabular-nums">
                        {pct.toFixed(1)}%
                      </span>
                      <span className="text-foreground/90 text-right text-[12px] font-medium tabular-nums">
                        {formatAmount(c.amount, currency)}
                      </span>
                    </div>
                  );
                })}
              </div>
            </>
          )}
        </div>
      </div>

      <Hr />

      {/* AFTER */}
      <SubLabel right={`${days}D EVENT WINDOW`}>AFTER · DID YOUR RHYTHM RETURN?</SubLabel>
      <div className="mt-3 grid grid-cols-1 gap-2.5 md:grid-cols-3">
        <RhythmCard
          label="7D BEFORE"
          value={beforeAvg}
          currency={currency}
          series={beforeSeries}
          accent="muted"
        />
        <RhythmCard
          label="DURING"
          value={dailyDuring}
          currency={currency}
          series={dailySeries}
          accent="during"
        />
        <RhythmCard
          label="3D AFTER"
          value={afterAvg}
          currency={currency}
          series={afterSeries}
          accent={hangoverPct > 5 ? "warn" : hangoverPct < -5 ? "good" : "muted"}
          hangoverPct={afterSeries.length > 0 ? hangoverPct : undefined}
        />
      </div>

      <Hr />

      {/* JUMP TO */}
      <SubLabel>JUMP TO</SubLabel>
      <div className="mt-2 flex flex-wrap gap-1.5">
        {events.map((ev) => {
          const c = getEventColors(ev);
          const isSel = ev.eventId === event.eventId;
          return (
            <button
              key={ev.eventId}
              type="button"
              onClick={() => onSelect(ev.eventId)}
              className={cn(
                "inline-flex items-center gap-2 rounded-full border px-3 py-1 text-[11px] transition-colors",
                isSel
                  ? "text-foreground"
                  : "text-muted-foreground hover:bg-muted/30 hover:text-foreground",
              )}
              style={{
                background: isSel ? c.fill : "transparent",
                borderColor: isSel ? c.stroke : "transparent",
              }}
            >
              <span
                className="inline-block h-2 w-2 rounded-[2px]"
                style={{ background: c.fill, border: `1.5px solid ${c.stroke}` }}
              />
              {ev.eventName}
              <span className="text-muted-foreground/80 ml-1">
                · {formatChipDate(new Date(ev.startDate))}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
};

function StatCell({
  label,
  divided,
  children,
}: {
  label: string;
  divided?: boolean;
  children: React.ReactNode;
}) {
  return (
    <div className={cn(divided && "md:border-border/40 md:border-l md:pl-4")}>
      <div className={LABEL_CLASS}>{label}</div>
      <div className="mt-1.5">{children}</div>
    </div>
  );
}

function SubLabel({ children, right }: { children: React.ReactNode; right?: string }) {
  return (
    <div className="mt-5 flex items-center justify-between gap-3">
      <div className={LABEL_CLASS}>{children}</div>
      {right ? <div className={cn(LABEL_CLASS, "text-right")}>{right}</div> : null}
    </div>
  );
}

function Hr() {
  return <div className="bg-border/40 my-5 h-px" />;
}

function DailyBars({
  beforeSeries,
  duringSeries,
  afterSeries,
  startDate,
  endDate,
  baseline,
  currency,
}: {
  beforeSeries: number[];
  duringSeries: number[];
  afterSeries: number[];
  startDate: Date;
  endDate: Date;
  baseline: number;
  currency: string;
}) {
  const max = Math.max(1, baseline, ...beforeSeries, ...duringSeries, ...afterSeries);
  const duringDays = duringSeries.length;

  const leftDate = useMemo(() => {
    if (beforeSeries.length === 0) return startDate;
    const d = new Date(startDate);
    d.setDate(d.getDate() - beforeSeries.length);
    return d;
  }, [startDate, beforeSeries.length]);
  const rightDate = useMemo(() => {
    if (afterSeries.length === 0) return endDate;
    const d = new Date(endDate);
    d.setDate(d.getDate() + afterSeries.length);
    return d;
  }, [endDate, afterSeries.length]);

  const segments: { key: string; data: number[]; className: string }[] = [];
  if (beforeSeries.length > 0)
    segments.push({ key: "before", data: beforeSeries, className: "bg-foreground/35" });
  segments.push({ key: "during", data: duringSeries, className: "bg-success/80" });
  if (afterSeries.length > 0)
    segments.push({ key: "after", data: afterSeries, className: "bg-foreground/35" });

  return (
    <div className="mt-3">
      <div className="relative flex h-28 items-end gap-2">
        {baseline > 0 && (
          <div
            className="border-foreground/30 pointer-events-none absolute left-0 right-0 border-t border-dashed"
            style={{ bottom: `${(baseline / max) * 100}%` }}
          />
        )}
        {segments.flatMap((seg, segIdx) => {
          const nodes = [
            <div
              key={seg.key}
              className="flex h-full items-end gap-[3px]"
              style={{ flex: seg.data.length }}
            >
              {seg.data.map((v, i) => (
                <div
                  key={i}
                  className={cn("min-w-[2px] flex-1 rounded-t-[2px]", seg.className)}
                  style={{ height: `${(Math.max(v, 0) / max) * 100}%` }}
                  title={formatAmount(v, currency)}
                />
              ))}
            </div>,
          ];
          if (segIdx > 0)
            nodes.unshift(
              <div key={`sep-${seg.key}`} className="bg-foreground/40 -my-1 w-px self-stretch" />,
            );
          return nodes;
        })}
      </div>
      <div className="text-muted-foreground/80 mt-2 flex items-center justify-between text-[10px] tracking-wide">
        <span className="tabular-nums">{formatPeakDay(leftDate)}</span>
        <span className="text-muted-foreground/60">
          {duringDays} day{duringDays === 1 ? "" : "s"}
        </span>
        <span className="tabular-nums">{formatPeakDay(rightDate)}</span>
      </div>
    </div>
  );
}

type RhythmAccent = "muted" | "during" | "warn" | "good";

function RhythmCard({
  label,
  value,
  currency,
  series,
  accent,
  hangoverPct,
}: {
  label: string;
  value: number;
  currency: string;
  series: number[];
  accent: RhythmAccent;
  hangoverPct?: number;
}) {
  const palette = {
    muted: {
      bg: "bg-muted/20",
      border: "border-border/40",
      stroke: "var(--muted-foreground)",
      fill: null as string | null,
    },
    during: {
      bg: "bg-success/10",
      border: "border-success/30",
      stroke: "var(--success)",
      fill: "var(--success)",
    },
    warn: {
      bg: "bg-destructive/10",
      border: "border-destructive/30",
      stroke: "var(--destructive)",
      fill: null as string | null,
    },
    good: {
      bg: "bg-success/10",
      border: "border-success/30",
      stroke: "var(--success)",
      fill: null as string | null,
    },
  }[accent];

  return (
    <div className={cn("rounded-md border px-3 py-2", palette.bg, palette.border)}>
      <div className="flex items-baseline justify-between gap-2">
        <div className={LABEL_CLASS}>{label}</div>
        {typeof hangoverPct === "number" && Math.abs(hangoverPct) > 5 && (
          <span
            className={cn(
              "rounded-sm px-1 py-0.5 text-[9px] tracking-wider",
              hangoverPct > 0 ? "bg-destructive/15 text-destructive" : "bg-success/15 text-success",
            )}
          >
            {hangoverPct > 0 ? `HANGOVER +${hangoverPct}%` : `UNDER ${hangoverPct}%`}
          </span>
        )}
      </div>
      <div className="mt-1 flex items-center justify-between gap-2">
        <span className="text-foreground text-[13px] font-medium tabular-nums">
          {series.length === 0 ? (
            "—"
          ) : (
            <>
              {formatAmount(value, currency)}
              <span className="text-muted-foreground/70 font-normal">/d</span>
            </>
          )}
        </span>
        {series.length > 0 && (
          <Sparkline data={series} stroke={palette.stroke} fill={palette.fill} />
        )}
      </div>
    </div>
  );
}

function Sparkline({
  data,
  stroke,
  fill,
}: {
  data: number[];
  stroke: string;
  fill: string | null;
}) {
  const w = 80;
  const h = 22;
  const max = Math.max(1, ...data);
  const pts = data.map((v, i) => {
    const x = (i / Math.max(1, data.length - 1)) * w;
    const y = h - (v / max) * h;
    return `${x},${y}`;
  });
  const line = `M${pts.join(" L")}`;
  const area = `${line} L${w},${h} L0,${h} Z`;
  return (
    <svg width={w} height={h} className="shrink-0">
      {fill && <path d={area} fill={fill} opacity={0.25} />}
      <path d={line} fill="none" stroke={stroke} strokeWidth={1.2} />
    </svg>
  );
}

// ─── EventDetailPanel data helpers ───────────────────────────────────────

function buildEventDailySeries(event: EventSpendingSummary, days: number): number[] {
  const start = new Date(event.startDate);
  const series = new Array(days).fill(0);
  for (const [dateKey, amount] of Object.entries(event.dailySpending ?? {})) {
    const d = new Date(`${dateKey}T12:00:00`);
    const idx = Math.round((d.getTime() - start.getTime()) / 86_400_000);
    if (idx >= 0 && idx < days) series[idx] = amount;
  }
  return series;
}

function findPeakDay(
  event: EventSpendingSummary,
  series: number[],
): { date: Date; amount: number } | null {
  let bestIdx = -1;
  let best = -Infinity;
  series.forEach((v, i) => {
    if (v > best) {
      best = v;
      bestIdx = i;
    }
  });
  if (bestIdx < 0 || best <= 0) return null;
  const start = new Date(event.startDate);
  const d = new Date(start);
  d.setDate(d.getDate() + bestIdx);
  return { date: d, amount: best };
}

function buildWindowSeries(
  activities: Activity[],
  accountTypeById: Map<string, string> | undefined,
  anchor: Date,
  offsetDays: number,
  windowDays: number,
): number[] {
  const start = new Date(anchor);
  start.setDate(start.getDate() + offsetDays);
  start.setHours(0, 0, 0, 0);
  const series = new Array(windowDays).fill(0);
  for (const a of activities) {
    const amt = getActivitySpendingAmount(a, accountTypeById?.get(a.accountId));
    if (amt <= 0) continue;
    const idx = Math.floor((new Date(a.activityDate).getTime() - start.getTime()) / 86_400_000);
    if (idx >= 0 && idx < windowDays) series[idx] += amt;
  }
  return series.some((v) => v > 0) ? series : [];
}

function avgSeries(series: number[]): number {
  if (series.length === 0) return 0;
  return series.reduce((a, b) => a + b, 0) / series.length;
}

function formatPeakDay(d: Date): string {
  const dayNames = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
  return `${dayNames[d.getDay()]}, ${MONTH_LABELS[d.getMonth()]} ${d.getDate()}`;
}

function formatChipDate(d: Date): string {
  return `${MONTH_LABELS[d.getMonth()]} ${d.getDate()}`;
}

interface EventCategoryRow {
  id: string;
  name: string;
  color: string;
  amount: number;
}

function buildEventCategoryRows(
  event: EventSpendingSummary,
  taxonomyCategories: TaxonomyCategory[],
): EventCategoryRow[] {
  const meta = new Map(taxonomyCategories.map((c) => [c.id, c]));
  const byTop = new Map<string, EventCategoryRow>();
  for (const cat of Object.values(event.byCategory)) {
    const m = cat.categoryId ? meta.get(cat.categoryId) : undefined;
    const topId = m?.parentId ?? cat.categoryId ?? cat.categoryName;
    const top = (m?.parentId && meta.get(m.parentId)) || m;
    const name = top?.name ?? cat.categoryName ?? "Uncategorized";
    const color = top?.color ?? cat.color ?? "#9CA3AF";
    const e = byTop.get(topId) ?? { id: topId, name, color, amount: 0 };
    e.amount += cat.amount;
    byTop.set(topId, e);
  }
  return Array.from(byTop.values())
    .filter((row) => row.amount > 0)
    .sort((a, b) => b.amount - a.amount);
}

function computeBaselinePace(
  activities: Activity[],
  excludeEvents: EventSpendingSummary[],
  accountTypeById?: Map<string, string>,
): number {
  const exclude = new Set<string>();
  for (const ev of excludeEvents) {
    const start = new Date(ev.startDate);
    const end = new Date(ev.endDate);
    for (let d = new Date(start); d <= end; d.setDate(d.getDate() + 1)) {
      exclude.add(d.toISOString().slice(0, 10));
    }
  }
  let total = 0;
  const seen = new Set<string>();
  for (const a of activities) {
    const spendingAmount = getActivitySpendingAmount(a, accountTypeById?.get(a.accountId));
    if (spendingAmount === 0) continue;
    const dayKey = new Date(a.activityDate).toISOString().slice(0, 10);
    if (exclude.has(dayKey)) continue;
    total += spendingAmount;
    seen.add(dayKey);
  }
  return seen.size === 0 ? 0 : Math.max(0, total) / seen.size;
}

function buildEventCaption({
  days,
  lift,
  currency,
  top,
}: {
  days: number;
  lift: number;
  currency: string;
  top: EventCategoryRow[];
}): string {
  if (top.length === 0) {
    return lift > 0
      ? `Lift vs your normal week: +${formatAmount(lift, currency)} over ${days} days.`
      : `In line with your normal week.`;
  }
  if (lift > 0 && days <= 4) {
    if (top.length === 1) {
      return `One-off — ${top[0].name} drove the spike.`;
    }
    return `One-off — ${top[0].name} and ${top[1].name} drove the spike.`;
  }
  if (Math.abs(lift) < 50) {
    return `Mostly ${top[0].name.toLowerCase()} — modest lift over a normal stretch.`;
  }
  return `Lift vs your normal week: ${lift >= 0 ? "+" : "−"}${formatAmount(Math.abs(lift), currency)} over ${days} days.`;
}

function pickEventIcon(name: string): typeof Icons.Calendar {
  const n = name.toLowerCase();
  if (n.includes("trip") || n.includes("travel") || n.includes("flight"))
    return Icons.Plane ?? Icons.Calendar;
  if (n.includes("birthday") || n.includes("party") || n.includes("celebration"))
    return Icons.Star ?? Icons.Calendar;
  if (n.includes("move") || n.includes("apartment") || n.includes("home"))
    return Icons.Home ?? Icons.Calendar;
  if (n.includes("wedding")) return Icons.Sparkles ?? Icons.Calendar;
  return Icons.Calendar;
}

function formatRange(start: Date, end: Date): string {
  const sameMonth =
    start.getMonth() === end.getMonth() && start.getFullYear() === end.getFullYear();
  return sameMonth
    ? `${formatMonthDay(start)}–${end.getDate()}`.toUpperCase()
    : `${formatMonthDay(start)} – ${formatMonthDay(end)}`.toUpperCase();
}

/** "2026-05-08" → "May 8" (parsed at noon to avoid UTC drift). */
function formatOutOfRangeDate(dateKey: string): string {
  return formatMonthDay(new Date(`${dateKey.slice(0, 10)}T12:00:00`));
}

// ═════════════════════════════════════════════════════════════════════════
// Empty events state
// ═════════════════════════════════════════════════════════════════════════

function EmptyEventsCard() {
  return (
    <div className={CARD_CLASS}>
      <p className="text-foreground text-base font-semibold leading-snug">
        No trips or events tagged
      </p>
      <p className="text-muted-foreground/80 mt-2 text-sm">
        Tag a trip, place, or one-off to see how it compares with your normal week.
      </p>
      <Button asChild variant="outline" size="sm" className="mt-4">
        <Link to="/activities?tab=spending">
          Tag event
          <Icons.ArrowRight className="ml-1.5 h-3.5 w-3.5" aria-hidden />
        </Link>
      </Button>
    </div>
  );
}
