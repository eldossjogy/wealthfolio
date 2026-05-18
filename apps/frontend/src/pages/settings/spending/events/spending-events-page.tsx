import { useMemo, useState } from "react";
import { Navigate, useNavigate } from "react-router-dom";

import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  AlertDialogTrigger,
  Button,
  Icons,
  Skeleton,
} from "@wealthfolio/ui";

import { useEventDialog } from "@/features/spending/components/event-dialog-provider";
import {
  useEventTypeMutations,
  useEventTypes,
  useSpendingEventMutations,
  useSpendingEvents,
} from "@/features/spending/hooks/use-spending-events";
import { useSpendingSettings } from "@/features/spending/hooks/use-spending-settings";
import { buildCashflowUrl } from "@/features/spending/lib/navigation";
import type { EventType, SpendingEvent } from "@/features/spending/types/event";

import { SettingsHeader } from "../../settings-header";
import { SpendingBackLink } from "../components/spending-back-link";

export default function SpendingEventsPage() {
  const { isEnabled, isLoading: settingsLoading } = useSpendingSettings();
  const navigate = useNavigate();
  const { data: events = [], isLoading: eventsLoading } = useSpendingEvents();
  const { data: eventTypes = [], isLoading: typesLoading } = useEventTypes();
  const { remove: removeEventType } = useEventTypeMutations();
  const { remove: removeEvent } = useSpendingEventMutations();
  const { openEventDialog, openEventTypeDialog } = useEventDialog();

  const [expandedTypes, setExpandedTypes] = useState<Set<string>>(new Set());

  const isLoading = eventsLoading || typesLoading;

  const eventsByType = useMemo(() => {
    const map: Record<string, SpendingEvent[]> = {};
    for (const e of events) {
      if (!map[e.eventTypeId]) map[e.eventTypeId] = [];
      map[e.eventTypeId].push(e);
    }
    return map;
  }, [events]);

  if (!settingsLoading && !isEnabled) {
    return <Navigate to="/settings/spending" replace />;
  }

  const toggleExpanded = (typeId: string) => {
    setExpandedTypes((prev) => {
      const next = new Set(prev);
      if (next.has(typeId)) next.delete(typeId);
      else next.add(typeId);
      return next;
    });
  };

  const handleAddEventType = () => {
    openEventTypeDialog();
  };

  const handleEditEventType = (eventType: EventType) => {
    openEventTypeDialog({ eventType });
  };

  const handleAddEvent = (eventType?: EventType) => {
    openEventDialog({
      prefill: { eventTypeId: eventType?.id },
    });
  };

  const handleEditEvent = (event: SpendingEvent) => {
    openEventDialog({ event });
  };

  return (
    <div className="space-y-6">
      <SpendingBackLink />
      <SettingsHeader
        heading="Events"
        text="Manage event types and events used to tag cash transactions."
        backTo="/settings/spending"
      >
        <Button onClick={handleAddEventType}>
          <Icons.Plus className="mr-2 h-4 w-4" />
          Add event type
        </Button>
      </SettingsHeader>

      {isLoading ? (
        <div className="space-y-2">
          <Skeleton className="h-12" />
          <Skeleton className="h-12" />
          <Skeleton className="h-12" />
        </div>
      ) : eventTypes.length > 0 ? (
        <div className="divide-border divide-y rounded-md border">
          {eventTypes.map((type) => {
            const typeEvents = eventsByType[type.id] ?? [];
            const hasEvents = typeEvents.length > 0;
            const isExpanded = expandedTypes.has(type.id);
            return (
              <div key={type.id}>
                <div className="flex items-center justify-between px-4 py-3">
                  <div className="flex items-center gap-3">
                    {hasEvents ? (
                      <Button
                        variant="ghost"
                        size="sm"
                        className="h-6 w-6 p-0"
                        onClick={() => toggleExpanded(type.id)}
                      >
                        {isExpanded ? (
                          <Icons.ChevronDown className="h-4 w-4" />
                        ) : (
                          <Icons.ChevronRight className="h-4 w-4" />
                        )}
                      </Button>
                    ) : (
                      <div className="w-6" />
                    )}
                    {type.color && (
                      <div
                        className="h-3 w-3 rounded-full"
                        style={{ backgroundColor: type.color }}
                      />
                    )}
                    <span className="font-medium">{type.name}</span>
                    {hasEvents && (
                      <span className="text-muted-foreground text-xs">({typeEvents.length})</span>
                    )}
                  </div>
                  <div className="flex items-center gap-1">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleAddEvent(type)}
                      title="Add event"
                    >
                      <Icons.Plus className="h-4 w-4" />
                    </Button>
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => handleEditEventType(type)}
                      title="Edit event type"
                    >
                      <Icons.Pencil className="h-4 w-4" />
                    </Button>
                    <AlertDialog>
                      <AlertDialogTrigger asChild>
                        <Button variant="ghost" size="sm" title="Delete event type">
                          <Icons.Trash className="h-4 w-4" />
                        </Button>
                      </AlertDialogTrigger>
                      <AlertDialogContent>
                        <AlertDialogHeader>
                          <AlertDialogTitle>Delete event type</AlertDialogTitle>
                          <AlertDialogDescription>
                            Are you sure you want to delete &quot;{type.name}&quot;?
                            {hasEvents && (
                              <span className="text-destructive mt-2 block font-medium">
                                This will also delete all {typeEvents.length} event(s) under this
                                type.
                              </span>
                            )}
                            This action cannot be undone.
                          </AlertDialogDescription>
                        </AlertDialogHeader>
                        <AlertDialogFooter>
                          <AlertDialogCancel>Cancel</AlertDialogCancel>
                          <AlertDialogAction
                            onClick={() => removeEventType.mutate(type.id)}
                            className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                          >
                            Delete
                          </AlertDialogAction>
                        </AlertDialogFooter>
                      </AlertDialogContent>
                    </AlertDialog>
                  </div>
                </div>
                {hasEvents && isExpanded && (
                  <div className="space-y-0">
                    {typeEvents.map((event) => (
                      <div key={event.id} className="ml-6 border-l pl-4">
                        <div className="flex items-center justify-between py-3">
                          <div className="flex items-center gap-3">
                            <div className="w-6" />
                            {type.color && (
                              <span
                                className="h-2.5 w-2.5 shrink-0 rounded-full"
                                style={{ backgroundColor: type.color }}
                                aria-hidden="true"
                              />
                            )}
                            <div>
                              <span className="text-sm">{event.name}</span>
                              <div className="text-muted-foreground flex items-center gap-2 text-xs">
                                <span>
                                  {event.startDate} – {event.endDate}
                                </span>
                              </div>
                            </div>
                          </div>
                          <div className="flex items-center gap-1">
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() =>
                                navigate(
                                  buildCashflowUrl({
                                    startDate: event.startDate,
                                    endDate: event.endDate,
                                  }),
                                )
                              }
                              title="View transactions"
                            >
                              <Icons.ExternalLink className="h-4 w-4" />
                            </Button>
                            <Button
                              variant="ghost"
                              size="sm"
                              onClick={() => handleEditEvent(event)}
                              title="Edit event"
                            >
                              <Icons.Pencil className="h-4 w-4" />
                            </Button>
                            <AlertDialog>
                              <AlertDialogTrigger asChild>
                                <Button variant="ghost" size="sm" title="Delete event">
                                  <Icons.Trash className="h-4 w-4" />
                                </Button>
                              </AlertDialogTrigger>
                              <AlertDialogContent>
                                <AlertDialogHeader>
                                  <AlertDialogTitle>Delete event</AlertDialogTitle>
                                  <AlertDialogDescription>
                                    Are you sure you want to delete &quot;{event.name}&quot;? This
                                    action cannot be undone.
                                  </AlertDialogDescription>
                                </AlertDialogHeader>
                                <AlertDialogFooter>
                                  <AlertDialogCancel>Cancel</AlertDialogCancel>
                                  <AlertDialogAction
                                    onClick={() => removeEvent.mutate(event.id)}
                                    className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
                                  >
                                    Delete
                                  </AlertDialogAction>
                                </AlertDialogFooter>
                              </AlertDialogContent>
                            </AlertDialog>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>
            );
          })}
        </div>
      ) : (
        <div className="text-muted-foreground py-8 text-center text-sm">
          No event types yet. Click &quot;Add event type&quot; to create one.
        </div>
      )}
    </div>
  );
}
