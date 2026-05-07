import { useState } from "react";
import { useNavigate } from "react-router-dom";

import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  Icons,
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@wealthfolio/ui";

import { useEventTypes, useSpendingEvents } from "../hooks/use-spending-events";

const EVENTS_MANAGE_PATH = "/settings/spending/events";

export interface QuickEventPopoverProps {
  trigger: React.ReactNode;
  selectedEventId?: string | null;
  onSelect: (eventId: string) => void;
  onClear?: () => void;
  align?: "start" | "center" | "end";
}

export function QuickEventPopover({
  trigger,
  selectedEventId,
  onSelect,
  onClear,
  align = "start",
}: QuickEventPopoverProps) {
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const { data: events = [] } = useSpendingEvents();
  const { data: eventTypes = [] } = useEventTypes();

  const handleCreate = () => {
    setOpen(false);
    navigate(EVENTS_MANAGE_PATH);
  };

  const typeById = new Map(eventTypes.map((t) => [t.id, t]));

  // Group events by their event type for a tidy list
  const groups = new Map<string, typeof events>();
  for (const e of events) {
    const arr = groups.get(e.eventTypeId) ?? [];
    arr.push(e);
    groups.set(e.eventTypeId, arr);
  }

  const handleSelect = (id: string) => {
    onSelect(id);
    setOpen(false);
  };
  const handleClear = () => {
    onClear?.();
    setOpen(false);
  };

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      <PopoverContent className="w-[280px] p-0" align={align}>
        <Command>
          <CommandInput placeholder="Search events..." />
          <CommandList>
            {events.length === 0 ? (
              <CommandEmpty>
                <div className="text-muted-foreground p-3 text-center text-xs">No events yet.</div>
              </CommandEmpty>
            ) : (
              <CommandEmpty>No events found.</CommandEmpty>
            )}
            {Array.from(groups.entries()).map(([typeId, items]) => {
              const t = typeById.get(typeId);
              return (
                <CommandGroup key={typeId} heading={t?.name ?? "Other"}>
                  {items.map((ev) => {
                    const isSelected = selectedEventId === ev.id;
                    return (
                      <CommandItem
                        key={ev.id}
                        value={`${t?.name ?? ""} ${ev.name}`}
                        onSelect={() => handleSelect(ev.id)}
                        className="flex items-center gap-2"
                      >
                        <span
                          className="h-2.5 w-2.5 shrink-0 rounded-full"
                          style={{ backgroundColor: t?.color ?? "var(--muted-foreground)" }}
                        />
                        <span className="min-w-0 flex-1 truncate">{ev.name}</span>
                        <span className="text-muted-foreground/70 shrink-0 text-[10px]">
                          {ev.startDate.slice(5)}
                        </span>
                        {isSelected && (
                          <Icons.Check className="text-muted-foreground h-3.5 w-3.5" />
                        )}
                      </CommandItem>
                    );
                  })}
                </CommandGroup>
              );
            })}
            <CommandSeparator />
            <CommandGroup>
              <CommandItem
                value="create-event"
                onSelect={handleCreate}
                className="text-primary flex items-center gap-2"
              >
                <Icons.Plus className="h-3.5 w-3.5" />
                Create event
              </CommandItem>
              {selectedEventId && onClear && (
                <CommandItem
                  value="clear-event"
                  onSelect={handleClear}
                  className="text-destructive hover:bg-destructive/10"
                >
                  <Icons.X className="mr-2 h-3.5 w-3.5" />
                  Clear event
                </CommandItem>
              )}
            </CommandGroup>
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
