import { ActivityType } from "@/lib/constants";
import { ActivityDetails, TimePeriod } from "@/lib/types";
import { cn, formatDate } from "@/lib/utils";
import { Icons, formatAmount } from "@wealthfolio/ui";
import { useMemo } from "react";
import { Area, AreaChart, ReferenceDot, ResponsiveContainer, Tooltip, YAxis } from "recharts";

interface ActivityEnrichment {
  activityType: ActivityType;
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
      const enriched = {
        id: act.id,
        activityType: act.activityType,
        quantity: act.quantity,
        unitPrice: act.unitPrice,
      };

      const key = dateKey(new Date(act.date));
      const list = activitiesByDate.get(key);
      if (list) {
        list.push(enriched);
      } else {
        activitiesByDate.set(key, [enriched]);
      }
    }

    const activityMarkers: ActivityMarker[] = [];
    const enrichedData: HistoryChartData[] = [];

    data.forEach((point, index) => {
      const key = dateKey(new Date(point.timestamp));
      const matchingActivities = activitiesByDate.get(key);

      if (matchingActivities) {
        enrichedData.push({ ...point, activities: matchingActivities });
        for (const act of matchingActivities) {
          activityMarkers.push({ index, act, point });
        }
      } else {
        enrichedData.push(point);
      }
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
              const shape =
                marker.act.activityType === "BUY" ? <Icons.BuyDot /> : <Icons.SellDot />;
              return (
                <ReferenceDot
                  r={10}
                  key={marker.act.id}
                  shape={shape}
                  x={marker.index}
                  y={marker.point.totalValue}
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
  point: HistoryChartData;
}

function dateKey(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}
