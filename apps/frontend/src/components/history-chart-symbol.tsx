import { ActivityDetails, TimePeriod } from "@/lib/types";
import { cn, formatDate } from "@/lib/utils";
import { formatAmount } from "@wealthfolio/ui";
import { useMemo } from "react";
import { Area, AreaChart, ReferenceDot, ResponsiveContainer, Tooltip, YAxis } from "recharts";

interface ActivityEnrichment {
  activityType: "BUY" | "SELL";
  quantity: string | null;
  unitPrice: string | null;
  id: string;
}

interface CustomTooltipProps<
  TPayload = {
    timestamp: string;
    currency: string;
    activities?: ActivityEnrichment[];
  },
> {
  active: boolean;
  payload: { value: number; payload: TPayload }[];
}

const CustomTooltip = ({ active, payload }: CustomTooltipProps) => {
  if (active && payload?.length) {
    const data = payload[0].payload;

    return (
      <div className="bg-popover text-popover-foreground border-border/60 rounded-lg border p-3 text-sm shadow-lg">
        <p className="text-muted-foreground mb-2 font-medium">{formatDate(data.timestamp)}</p>
        <p className="mb-2 text-lg font-bold">
          {formatAmount(payload[0].value, data.currency, false)}
        </p>
        {data.activities && data.activities.length > 0 && (
          <div className="flex flex-col gap-2">
            {data.activities.map((act) => {
              const isBuy = act.activityType === "BUY";
              return (
                <div
                  key={act.id}
                  className={cn(
                    "flex items-center gap-2.5 rounded-md border px-3 py-2",
                    isBuy
                      ? "border-green-500/30 bg-green-500/10"
                      : "border-blue-500/30 bg-blue-500/10",
                  )}
                >
                  <div className="flex flex-1 items-center justify-between gap-3">
                    <span
                      className={cn(
                        "text-sm font-semibold",
                        isBuy
                          ? "text-green-700 dark:text-green-300"
                          : "text-blue-700 dark:text-blue-300",
                      )}
                    >
                      {isBuy ? "Bought" : "Sold"}
                    </span>
                    <span className="text-muted-foreground text-sm tabular-nums">
                      {parseFloat(act.quantity || "0")} @{" "}
                      {formatAmount(parseFloat(act.unitPrice || "0"), data.currency, false)}
                    </span>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    );
  }

  return null;
};

interface HistoryChartData {
  timestamp: string;
  totalValue: number;
  currency: string;
  activities?: ActivityEnrichment[];
}

const BuyDot = ({ cx, cy }: { cx?: number; cy?: number }) => (
  <g transform={`translate(${cx}, ${cy})`}>
    <circle r={18} fill="var(--success)" opacity={0.15} />
    <circle r={10} fill="var(--success)" />
    <text
      x={0}
      y={0}
      textAnchor="middle"
      dominantBaseline="central"
      fill="white"
      fontSize={12}
      fontWeight="bold"
    >
      B
    </text>
  </g>
);

const SellDot = ({ cx, cy }: { cx?: number; cy?: number }) => (
  <g transform={`translate(${cx}, ${cy})`}>
    <circle r={18} fill="#3b82f6" opacity={0.15} />
    <circle r={10} fill="#3b82f6" />
    <text
      x={0}
      y={0}
      textAnchor="middle"
      dominantBaseline="central"
      fill="white"
      fontSize={12}
      fontWeight="bold"
    >
      S
    </text>
  </g>
);

// TODO: Clean up the file if we're okay to upstream the changes.
export default function HistoryChart({
  data,
  interval,
  activity,
  height = 350,
}: {
  data: HistoryChartData[];
  interval?: TimePeriod;
  height?: number;
  activity?: ActivityDetails[];
}) {
  const { enrichedData, activityMarkers } = useMemo(() => {
    if (!activity?.length) {
      return { enrichedData: data, activityMarkers: [] as ActivityMarker[] };
    }
    const activitiesByDate = new Map<string, ActivityEnrichment[]>();
    for (const act of activity) {
      if (!act.date) continue;
      const actDate = new Date(act.date);
      const dateKey = actDate.toISOString().split("T")[0];

      const existing = activitiesByDate.get(dateKey);
      if (existing) {
        existing.push(getEnrichedAct(act));
      } else {
        activitiesByDate.set(dateKey, [getEnrichedAct(act)]);
      }
    }

    const activityMarkers: ActivityMarker[] = [];
    const enrichedData: HistoryChartData[] = [];

    data.forEach((point, index) => {
      const pointDate = new Date(point.timestamp);
      const dateKey = pointDate.toISOString().split("T")[0];

      const matchingActivities = activitiesByDate.get(dateKey) || [];
      const final = { ...point };

      if (matchingActivities.length > 0) {
        final.activities = matchingActivities;
        matchingActivities.forEach((act) => {
          const m = { index, act, pointData: point };
          activityMarkers.push(m);
        });
      }
      enrichedData.push(final);
    });

    return { enrichedData, activityMarkers };
  }, [data, activity]);

  return (
    <div className="relative flex h-full flex-col" data-no-swipe-drag>
      <div className="grow">
        <ResponsiveContainer width="100%" height="100%" minHeight={height}>
          <AreaChart
            data={enrichedData}
            stackOffset="sign"
            margin={{
              top: 0,
              right: 0,
              left: 0,
              bottom: 0,
            }}
          >
            <defs>
              <linearGradient id="colorUv" x1="0" y1="0" x2="0" y2="1">
                <stop offset="5%" stopColor="var(--success)" stopOpacity={0.2} />
                <stop offset="95%" stopColor="var(--success)" stopOpacity={0.1} />
              </linearGradient>
            </defs>
            {/* @ts-expect-error - Recharts Tooltip content typing mismatch */}
            <Tooltip content={<CustomTooltip />} />
            {interval !== "ALL" && interval !== "1Y" ? (
              <YAxis hide={true} type="number" domain={["auto", "auto"]} />
            ) : null}
            <Area
              isAnimationActive={true}
              animationDuration={300}
              animationEasing="ease-out"
              connectNulls={true}
              type="monotone"
              dataKey="totalValue"
              stroke="var(--success)"
              fillOpacity={1}
              fill="url(#colorUv)"
            />
            {activityMarkers.map((marker) => {
              const shape = marker.act.activityType === "BUY" ? <BuyDot /> : <SellDot />;
              return (
                <ReferenceDot
                  key={marker.act.id}
                  x={marker.index}
                  y={marker.pointData.totalValue}
                  r={10}
                  shape={shape}
                />
              );
            })}
          </AreaChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}

interface ActivityMarker {
  index: number;
  act: ActivityEnrichment;
  pointData: HistoryChartData;
}

function getEnrichedAct(act: ActivityDetails): ActivityEnrichment {
  return {
    id: act.id,
    activityType: act.activityType as "BUY",
    quantity: act.quantity,
    unitPrice: act.unitPrice,
  };
}
