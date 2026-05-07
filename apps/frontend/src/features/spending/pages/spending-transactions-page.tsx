import { useCallback, useMemo, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { useNavigate, useSearchParams } from "react-router-dom";
import type { DateRange } from "react-day-picker";

import { createActivity, deleteActivity } from "@/adapters";
import { generateId } from "@/lib/id";
import { useAccounts } from "@/hooks/use-accounts";
import { useDebouncedValue } from "@/hooks/use-debounced-value";
import { useTaxonomy } from "@/hooks/use-taxonomies";
import { QueryKeys } from "@/lib/query-keys";
import type { Account, TaxonomyCategory } from "@/lib/types";

import {
  Button,
  EmptyPlaceholder,
  Icons,
  Page,
  PageContent,
  PageHeader,
  Skeleton,
  Table,
  TableBody,
  TableHead,
  TableHeader,
  TableRow,
  Checkbox,
} from "@wealthfolio/ui";

import { CashActivityForm } from "../components/cash-activity-form";
import type { AmountRange } from "../components/amount-range-filter";
import {
  DeleteTransactionsDialog,
  type DeletePreview,
} from "../components/delete-transactions-dialog";
import { TransactionRow } from "../components/transaction-row";
import { TransactionsBulkBar } from "../components/transactions-bulk-bar";
import { TransactionsFilterBar, type FilterOption } from "../components/transactions-filter-bar";
import { CASH_ACTIVITY_TYPES, CASH_ACTIVITY_TYPE_LABELS } from "../lib/constants";
import {
  pluralizeActivity,
  stableArr,
  toRowVM,
  type TransactionRowVM,
} from "../lib/transactions-helpers";
import { useCashActivitySearch } from "../hooks/use-cash-activity-search";
import {
  useAssignActivityCategory,
  useSetActivityEvent,
  useUnassignActivityCategory,
} from "../hooks/use-cash-activities";
import { useEventTypes, useSpendingEvents } from "../hooks/use-spending-events";
import type { CashActivitySearchRequest, CashActivityStatusFilter } from "../types/cash-activity";

const SPENDING_TAXONOMY = "spending_categories";
const INCOME_TAXONOMY = "income_sources";
const SEARCH_DEBOUNCE_MS = 300;

export default function SpendingTransactionsPage() {
  const [searchParams] = useSearchParams();
  const urlCategoryId = searchParams.get("category");
  const urlSubcategoryId = searchParams.get("subcategory");
  const urlStartDate = searchParams.get("from");
  const urlEndDate = searchParams.get("to");
  const urlStatus = searchParams.get("status") as CashActivityStatusFilter | null;

  const qc = useQueryClient();
  const navigate = useNavigate();

  // Sheet + delete-dialog state
  const [editingActivity, setEditingActivity] = useState<TransactionRowVM | undefined>();
  const [showForm, setShowForm] = useState(false);
  const [deletingIds, setDeletingIds] = useState<string[] | null>(null);
  const [deletePreview, setDeletePreview] = useState<DeletePreview | undefined>();

  // Search (raw input + debounced value sent to server)
  const [searchInput, setSearchInput] = useState("");
  const debouncedSearch = useDebouncedValue(searchInput.trim(), SEARCH_DEBOUNCE_MS);

  // Filter state
  const [statusFilter, setStatusFilter] = useState<CashActivityStatusFilter>(
    urlStatus &&
      (urlStatus === "all" ||
        urlStatus === "needs_review" ||
        urlStatus === "uncategorized" ||
        urlStatus === "categorized")
      ? urlStatus
      : "all",
  );
  const [selectedTypes, setSelectedTypes] = useState<Set<string>>(new Set());
  const [selectedAccounts, setSelectedAccounts] = useState<Set<string>>(new Set());
  const [selectedCategories, setSelectedCategories] = useState<Set<string>>(
    () => new Set(urlCategoryId ? [urlCategoryId] : []),
  );
  const [selectedSubcategories, setSelectedSubcategories] = useState<Set<string>>(
    () => new Set(urlSubcategoryId ? [urlSubcategoryId] : []),
  );
  const [selectedEvents, setSelectedEvents] = useState<Set<string>>(new Set());
  const [amountRange, setAmountRange] = useState<AmountRange>({ min: null, max: null });
  const [dateRange, setDateRange] = useState<DateRange | undefined>(() => {
    if (urlStartDate || urlEndDate) {
      return {
        from: urlStartDate ? new Date(urlStartDate) : undefined,
        to: urlEndDate ? new Date(urlEndDate) : undefined,
      };
    }
    return undefined;
  });

  // Selection
  const [selectedRowIds, setSelectedRowIds] = useState<Set<string>>(new Set());

  const { accounts = [] } = useAccounts({ filterActive: true });
  const cashAccounts = useMemo(
    () => accounts.filter((a: Account) => a.accountType === "CASH"),
    [accounts],
  );
  const { data: events = [] } = useSpendingEvents();
  const { data: eventTypes = [] } = useEventTypes();
  const spending = useTaxonomy(SPENDING_TAXONOMY);
  const income = useTaxonomy(INCOME_TAXONOMY);
  const assignMutation = useAssignActivityCategory();
  const unassignMutation = useUnassignActivityCategory();
  const setEventMutation = useSetActivityEvent();

  const allCategories = useMemo(() => {
    const map = new Map<string, TaxonomyCategory>();
    (spending.data?.categories ?? []).forEach((c) => map.set(c.id, c));
    (income.data?.categories ?? []).forEach((c) => map.set(c.id, c));
    return map;
  }, [spending.data?.categories, income.data?.categories]);

  const topLevelCategories = useMemo(
    () =>
      Array.from(allCategories.values())
        .filter((c) => !c.parentId)
        .sort((a, b) => a.sortOrder - b.sortOrder),
    [allCategories],
  );

  const subcategoriesForFilter = useMemo(() => {
    const all = Array.from(allCategories.values()).filter((c) => !!c.parentId);
    if (selectedCategories.size === 0) return all;
    return all.filter((c) => c.parentId && selectedCategories.has(c.parentId));
  }, [allCategories, selectedCategories]);

  // Expand top-level category selections to descendants — backend matches assignment.category_id exactly.
  const expandedCategoryIds = useMemo(() => {
    if (selectedCategories.size === 0) return undefined;
    const out = new Set<string>(selectedCategories);
    allCategories.forEach((c) => {
      if (c.parentId && selectedCategories.has(c.parentId)) out.add(c.id);
    });
    return [...out].sort();
  }, [selectedCategories, allCategories]);

  // Build the backend request — Sets are converted to sorted arrays so reordering
  // doesn't change the React Query key (would otherwise cause spurious refetches).
  const searchRequest: Omit<CashActivitySearchRequest, "offset" | "limit"> = useMemo(() => {
    return {
      search: debouncedSearch || undefined,
      accountIds: stableArr(selectedAccounts),
      activityTypes: stableArr(selectedTypes),
      categoryIds: expandedCategoryIds,
      subcategoryIds: stableArr(selectedSubcategories),
      eventIds: stableArr(selectedEvents),
      status: statusFilter,
      startDate: dateRange?.from ? dateRange.from.toISOString() : undefined,
      endDate: dateRange?.to
        ? (() => {
            const end = new Date(dateRange.to);
            end.setHours(23, 59, 59, 999);
            return end.toISOString();
          })()
        : undefined,
      minAmount: amountRange.min ?? undefined,
      maxAmount: amountRange.max ?? undefined,
      sortBy: "date",
      sortDir: "desc",
    };
  }, [
    debouncedSearch,
    selectedAccounts,
    selectedTypes,
    expandedCategoryIds,
    selectedSubcategories,
    selectedEvents,
    statusFilter,
    dateRange,
    amountRange,
  ]);

  const {
    items,
    totalCount,
    isLoading,
    isFetching,
    isFetchingNextPage,
    hasNextPage,
    fetchNextPage,
  } = useCashActivitySearch(searchRequest);

  // Lookup maps
  const accountById = useMemo(() => {
    const m = new Map<string, Account>();
    cashAccounts.forEach((a) => m.set(a.id, a));
    return m;
  }, [cashAccounts]);

  const eventsById = useMemo(() => new Map(events.map((e) => [e.id, e])), [events]);
  const eventTypeById = useMemo(() => new Map(eventTypes.map((t) => [t.id, t])), [eventTypes]);

  // Row VMs
  const rows: TransactionRowVM[] = useMemo(
    () => items.map((it) => toRowVM(it, allCategories)),
    [items, allCategories],
  );

  // Active-filter detection — uses debouncedSearch (the value actually applied)
  const filtersActive =
    !!debouncedSearch ||
    statusFilter !== "all" ||
    selectedTypes.size > 0 ||
    selectedAccounts.size > 0 ||
    selectedCategories.size > 0 ||
    selectedSubcategories.size > 0 ||
    selectedEvents.size > 0 ||
    amountRange.min != null ||
    amountRange.max != null ||
    !!dateRange?.from ||
    !!dateRange?.to;

  const clearAllFilters = useCallback(() => {
    setSearchInput("");
    setStatusFilter("all");
    setSelectedTypes(new Set());
    setSelectedAccounts(new Set());
    setSelectedCategories(new Set());
    setSelectedSubcategories(new Set());
    setSelectedEvents(new Set());
    setAmountRange({ min: null, max: null });
    setDateRange(undefined);
  }, []);

  // Reset selection when applied filters change. Compare-during-render pattern
  // (React docs: "Resetting state when a prop changes") — avoids an effect roundtrip.
  // Sorted-set keys mean toggling filter order doesn't trigger a reset.
  const requestKey = useMemo(() => JSON.stringify(searchRequest), [searchRequest]);
  const [lastRequestKey, setLastRequestKey] = useState(requestKey);
  if (lastRequestKey !== requestKey) {
    setLastRequestKey(requestKey);
    setSelectedRowIds(new Set());
  }

  // Mutations
  const duplicateMutation = useMutation({
    mutationFn: async (row: TransactionRowVM) => {
      const a = row.activity;
      return createActivity({
        idempotencyKey: generateId("manual-duplicate"),
        accountId: a.accountId,
        activityType: a.activityType,
        currency: a.currency,
        amount: a.amount,
        activityDate:
          typeof a.activityDate === "string" ? a.activityDate : new Date().toISOString(),
        comment: "Duplicated",
      });
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: [QueryKeys.SPENDING_TRANSACTIONS] });
      qc.invalidateQueries({ queryKey: [QueryKeys.SPENDING_SUMMARY] });
      qc.invalidateQueries({ queryKey: [QueryKeys.ACTIVITIES] });
      toast.success("Transaction duplicated.");
    },
    onError: () => toast.error("Failed to duplicate transaction."),
  });

  const handleDuplicate = useCallback(
    (row: TransactionRowVM) => duplicateMutation.mutate(row),
    [duplicateMutation],
  );

  const deleteMutation = useMutation({
    mutationFn: async (ids: string[]) => {
      const results = await Promise.allSettled(ids.map((id) => deleteActivity(id)));
      return results;
    },
    onSuccess: (results) => {
      qc.invalidateQueries({ queryKey: [QueryKeys.SPENDING_TRANSACTIONS] });
      qc.invalidateQueries({ queryKey: [QueryKeys.SPENDING_SUMMARY] });
      qc.invalidateQueries({ queryKey: [QueryKeys.ACTIVITIES] });
      const ok = results.filter((r) => r.status === "fulfilled").length;
      const failed = results.length - ok;
      if (ok > 0) toast.success(`Deleted ${ok} ${pluralizeActivity(ok)}.`);
      if (failed > 0) toast.error(`Failed to delete ${failed} ${pluralizeActivity(failed)}.`);
      setDeletingIds(null);
      setDeletePreview(undefined);
      setSelectedRowIds(new Set());
    },
    onError: () => toast.error("Failed to delete activities."),
  });

  // Bulk handlers — Promise.allSettled so partial failures are reported
  const handleBulkCategorize = useCallback(
    async (taxonomyId: string, categoryId: string) => {
      const ids = Array.from(selectedRowIds);
      const results = await Promise.allSettled(
        ids.map((activityId) => assignMutation.mutateAsync({ activityId, taxonomyId, categoryId })),
      );
      const ok = results.filter((r) => r.status === "fulfilled").length;
      const failed = results.length - ok;
      if (ok > 0) toast.success(`Categorized ${ok} ${pluralizeActivity(ok)}.`);
      if (failed > 0) toast.error(`Failed to categorize ${failed}.`);
      setSelectedRowIds(new Set());
    },
    [selectedRowIds, assignMutation],
  );

  const handleBulkSetEvent = useCallback(
    async (eventId: string | null) => {
      const ids = Array.from(selectedRowIds);
      const results = await Promise.allSettled(
        ids.map((activityId) => setEventMutation.mutateAsync({ activityId, eventId })),
      );
      const ok = results.filter((r) => r.status === "fulfilled").length;
      const failed = results.length - ok;
      const verb = eventId ? "Tagged" : "Cleared event from";
      if (ok > 0) toast.success(`${verb} ${ok} ${pluralizeActivity(ok)}.`);
      if (failed > 0) toast.error(`Failed on ${failed} ${pluralizeActivity(failed)}.`);
      setSelectedRowIds(new Set());
    },
    [selectedRowIds, setEventMutation],
  );

  const clearSelection = useCallback(() => setSelectedRowIds(new Set()), []);

  // Inline handlers — stable refs so memoized rows don't re-render unnecessarily
  const handleAssignCategory = useCallback(
    (activityId: string, taxonomyId: string, categoryId: string) => {
      assignMutation.mutate({ activityId, taxonomyId, categoryId });
    },
    [assignMutation],
  );
  const handleClearCategory = useCallback(
    (activityId: string, taxonomyId: string) => {
      unassignMutation.mutate({ activityId, taxonomyId });
    },
    [unassignMutation],
  );
  const handleSetEvent = useCallback(
    (activityId: string, eventId: string | null) => {
      setEventMutation.mutate({ activityId, eventId });
    },
    [setEventMutation],
  );

  const handleEditRow = useCallback((row: TransactionRowVM) => {
    setEditingActivity(row);
    setShowForm(true);
  }, []);
  const handleDeleteRow = useCallback((row: TransactionRowVM) => {
    setDeletingIds([row.activity.id]);
    setDeletePreview({
      activityType: row.activity.activityType,
      amount: row.activity.amount ?? null,
      currency: row.activity.currency,
    });
  }, []);

  const handleToggleRow = useCallback((id: string) => {
    setSelectedRowIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  }, []);

  // Header checkbox state — rebuilt as derived value
  const allVisibleSelected =
    rows.length > 0 && rows.every((r) => selectedRowIds.has(r.activity.id));
  const someVisibleSelected =
    rows.some((r) => selectedRowIds.has(r.activity.id)) && !allVisibleSelected;

  const toggleSelectAllVisible = () => {
    setSelectedRowIds((prev) => {
      const next = new Set(prev);
      if (allVisibleSelected) rows.forEach((r) => next.delete(r.activity.id));
      else rows.forEach((r) => next.add(r.activity.id));
      return next;
    });
  };

  const handleBulkDelete = () => {
    setDeletingIds(Array.from(selectedRowIds));
    setDeletePreview(undefined);
  };

  // Memoized faceted-filter option lists
  const typeOptions = useMemo<FilterOption[]>(
    () =>
      CASH_ACTIVITY_TYPES.map((t) => ({
        value: t,
        label: CASH_ACTIVITY_TYPE_LABELS[t],
      })),
    [],
  );
  const accountOptions = useMemo<FilterOption[]>(
    () => cashAccounts.map((a) => ({ value: a.id, label: a.name })),
    [cashAccounts],
  );
  const categoryOptions = useMemo<FilterOption[]>(
    () => topLevelCategories.map((c) => ({ value: c.id, label: c.name })),
    [topLevelCategories],
  );
  const subcategoryOptions = useMemo<FilterOption[]>(
    () =>
      subcategoriesForFilter.map((c) => {
        const parent = c.parentId ? allCategories.get(c.parentId) : null;
        return {
          value: c.id,
          label: parent ? `${parent.name} / ${c.name}` : c.name,
        };
      }),
    [subcategoriesForFilter, allCategories],
  );
  const eventOptions = useMemo<FilterOption[]>(
    () => events.map((e) => ({ value: e.id, label: e.name })),
    [events],
  );

  // Cascading reset: drop subcategories whose parent was deselected
  const handleCategoriesChange = useCallback(
    (next: Set<string>) => {
      setSelectedCategories(next);
      setSelectedSubcategories((prev) => {
        const drop = new Set<string>();
        prev.forEach((id) => {
          const cat = allCategories.get(id);
          if (!cat?.parentId || !next.has(cat.parentId)) drop.add(id);
        });
        if (drop.size === 0) return prev;
        const out = new Set(prev);
        drop.forEach((id) => out.delete(id));
        return out;
      });
    },
    [allCategories],
  );

  const headerActions = (
    <Button
      onClick={() => {
        setEditingActivity(undefined);
        setShowForm(true);
      }}
    >
      <Icons.Plus className="mr-2 h-4 w-4" aria-hidden="true" />
      Add transaction
    </Button>
  );

  const isRefreshing = isFetching && !isFetchingNextPage;

  // Memoized so the form's reset-effect doesn't fire on unrelated parent renders.
  const editingActivityForForm = useMemo(() => {
    if (!editingActivity) return undefined;
    const a = editingActivity.activity;
    const c = editingActivity.category;
    return c
      ? {
          ...a,
          categoryAssignmentId: c.assignmentId,
          categoryTaxonomyId: c.taxonomyId,
          categoryId: c.id,
        }
      : a;
  }, [editingActivity]);

  return (
    <Page>
      <PageHeader
        heading="Transactions"
        text="Cash activity across your tracked spending accounts."
        onBack={() => {
          // Prefer browser history; fall back to the spending hub for direct landings.
          if (window.history.length > 1) navigate(-1);
          else navigate("/spending");
        }}
        actions={headerActions}
      />
      <PageContent className="space-y-4 pb-2 md:pb-4 lg:pb-5">
        <TransactionsFilterBar
          searchInput={searchInput}
          onSearchInputChange={setSearchInput}
          statusFilter={statusFilter}
          onStatusFilterChange={setStatusFilter}
          dateRange={dateRange}
          onDateRangeChange={setDateRange}
          selectedAccounts={selectedAccounts}
          onAccountsChange={setSelectedAccounts}
          selectedTypes={selectedTypes}
          onTypesChange={setSelectedTypes}
          selectedCategories={selectedCategories}
          onCategoriesChange={handleCategoriesChange}
          selectedSubcategories={selectedSubcategories}
          onSubcategoriesChange={setSelectedSubcategories}
          selectedEvents={selectedEvents}
          onEventsChange={setSelectedEvents}
          amountRange={amountRange}
          onAmountRangeChange={setAmountRange}
          accountOptions={accountOptions}
          typeOptions={typeOptions}
          categoryOptions={categoryOptions}
          subcategoryOptions={subcategoryOptions}
          eventOptions={eventOptions}
          hasEvents={events.length > 0}
          filtersActive={filtersActive}
          onClearAll={clearAllFilters}
          visibleCount={rows.length}
          totalCount={totalCount}
          isRefreshing={isRefreshing}
        />

        {selectedRowIds.size > 0 && (
          <TransactionsBulkBar
            selectedCount={selectedRowIds.size}
            onCategorize={handleBulkCategorize}
            onTagEvent={handleBulkSetEvent}
            onDelete={handleBulkDelete}
            onClearSelection={clearSelection}
          />
        )}

        {isLoading ? (
          <div className="space-y-2">
            <Skeleton className="h-12" />
            <Skeleton className="h-12" />
            <Skeleton className="h-12" />
          </div>
        ) : rows.length === 0 ? (
          <EmptyPlaceholder>
            <EmptyPlaceholder.Icon name="Activity" />
            <EmptyPlaceholder.Title>No transactions</EmptyPlaceholder.Title>
            <EmptyPlaceholder.Description>
              {filtersActive
                ? "No cash activity matches your filters."
                : "Add your first transaction to get started."}
            </EmptyPlaceholder.Description>
            {filtersActive ? (
              <Button variant="outline" onClick={clearAllFilters}>
                Clear filters
              </Button>
            ) : (
              <Button
                onClick={() => {
                  setEditingActivity(undefined);
                  setShowForm(true);
                }}
              >
                <Icons.Plus className="mr-2 h-4 w-4" aria-hidden="true" />
                Add transaction
              </Button>
            )}
          </EmptyPlaceholder>
        ) : (
          <div className="rounded-md border">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead className="w-10">
                    <Checkbox
                      checked={
                        allVisibleSelected ? true : someVisibleSelected ? "indeterminate" : false
                      }
                      onCheckedChange={toggleSelectAllVisible}
                      aria-label={
                        allVisibleSelected
                          ? "Deselect all visible transactions"
                          : "Select all visible transactions"
                      }
                    />
                  </TableHead>
                  <TableHead>Date</TableHead>
                  <TableHead>Type</TableHead>
                  <TableHead>Account</TableHead>
                  <TableHead>Name / Notes</TableHead>
                  <TableHead>Category</TableHead>
                  <TableHead>Event</TableHead>
                  <TableHead className="text-right">Amount</TableHead>
                  <TableHead className="w-12" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map((r) => {
                  const eventId = r.activity.eventId ?? null;
                  const ev = eventId ? eventsById.get(eventId) : null;
                  const eventTypeColor = ev
                    ? (eventTypeById.get(ev.eventTypeId)?.color ?? null)
                    : null;
                  return (
                    <TransactionRow
                      key={r.activity.id}
                      row={r}
                      account={accountById.get(r.activity.accountId)}
                      event={ev ?? null}
                      eventTypeColor={eventTypeColor}
                      isSelected={selectedRowIds.has(r.activity.id)}
                      onToggleSelect={handleToggleRow}
                      onAssignCategory={handleAssignCategory}
                      onClearCategory={handleClearCategory}
                      onSetEvent={handleSetEvent}
                      onEdit={handleEditRow}
                      onDuplicate={handleDuplicate}
                      onDelete={handleDeleteRow}
                    />
                  );
                })}
              </TableBody>
            </Table>

            {hasNextPage && (
              <div className="border-border flex items-center justify-center border-t p-3">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => fetchNextPage()}
                  disabled={isFetchingNextPage}
                >
                  {isFetchingNextPage ? (
                    <>
                      <Icons.Spinner className="mr-2 h-4 w-4 animate-spin" aria-hidden="true" />
                      Loading…
                    </>
                  ) : (
                    `Load more (${totalCount - rows.length} remaining)`
                  )}
                </Button>
              </div>
            )}
          </div>
        )}

        <CashActivityForm
          open={showForm}
          onOpenChange={setShowForm}
          activity={editingActivityForForm}
        />

        <DeleteTransactionsDialog
          open={!!deletingIds && deletingIds.length > 0}
          count={deletingIds?.length ?? 0}
          preview={deletePreview}
          isPending={deleteMutation.isPending}
          onCancel={() => {
            setDeletingIds(null);
            setDeletePreview(undefined);
          }}
          onConfirm={() => deletingIds && deleteMutation.mutate(deletingIds)}
        />
      </PageContent>
    </Page>
  );
}
