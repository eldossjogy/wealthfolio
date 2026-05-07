import { useMemo } from "react";

import { useEventTypes } from "@/features/spending/hooks/use-spending-events";

import { OverviewCard, type OverviewChip } from "./overview-card";

export function EventTypesOverviewCard() {
  const { data: eventTypes = [], isLoading } = useEventTypes();

  const chips = useMemo<OverviewChip[]>(
    () =>
      eventTypes.map((t) => ({
        id: t.id,
        name: t.name,
        color: t.color ?? null,
      })),
    [eventTypes],
  );

  const total = eventTypes.length;

  return (
    <OverviewCard
      title="Event types"
      description={
        total === 0
          ? "Tag recurring or one-off events on transactions and timelines."
          : `${total} type${total === 1 ? "" : "s"} · used to tag transactions and timelines`
      }
      chips={chips}
      manageHref="/settings/spending/events"
      emptyTitle="No event types yet"
      emptyDescription="Add types like Vacation, Move, or Wedding to tag transactions."
      emptyCtaLabel="Add event type"
      isLoading={isLoading}
    />
  );
}
