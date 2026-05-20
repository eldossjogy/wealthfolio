import { memo, useCallback, useMemo, useState } from "react";

import { Icons, PrivacyAmount, Skeleton } from "@wealthfolio/ui";
import type { TaxonomyCategory } from "@/lib/types";
import { cn } from "@/lib/utils";

import { CategoryIcon } from "../category-chips";
import type { BudgetCategoryRow, BudgetGroupRow } from "../../types/budget";
import type { CategoryBreakdownRow } from "../../types/report";
import {
  buildGroupNodes,
  buildTree,
  formatDelta,
  type CategorySort,
  type GroupNode,
  type NodeRow,
} from "./category-hierarchy/builders";

export type { CategorySort };

interface CategoryHierarchyTableProps {
  /** Spending breakdown for the current period (flat rows from backend). */
  breakdown: CategoryBreakdownRow[];
  /** Prior-period breakdown — drives the Δ column. */
  priorBreakdown: CategoryBreakdownRow[];
  /** Backend-computed category budget rows. */
  budgetRows: BudgetCategoryRow[];
  /**
   * Budget groups (Needs / Wants / ...). When present, the table renders a
   * group-level wrapper above categories. When empty / undefined, the table
   * falls back to the original 2-level (category → subcategory) layout.
   */
  groupRows?: BudgetGroupRow[];
  /** Taxonomy metadata (used to resolve names + parent ids). */
  taxonomyCategories: TaxonomyCategory[];
  currency: string;
  isLoading: boolean;
  /** Sort order for top-level rows. Defaults to "spent" (largest first). */
  sort?: CategorySort;
  /** Fired when a category row is clicked (excluding the parent expand chevron). */
  onCategoryClick?: (categoryId: string) => void;
}

/**
 * Hierarchical Budgeted / Spent / Balance / Δ table.
 *
 * Without `groupRows`: rolls flat backend rows into a top-level → subcategory
 * tree using the taxonomy `parentId` graph.
 *
 * With `groupRows`: wraps the same tree in budget groups (Needs / Wants / …),
 * with a synthetic "Other" group catching categories that aren't assigned to
 * any group.
 */
export function CategoryHierarchyTable({
  breakdown,
  priorBreakdown,
  budgetRows,
  groupRows,
  taxonomyCategories,
  currency,
  isLoading,
  sort = "spent",
  onCategoryClick,
}: CategoryHierarchyTableProps) {
  const tree = useMemo(
    () => buildTree({ breakdown, priorBreakdown, budgetRows, taxonomyCategories, sort }),
    [breakdown, priorBreakdown, budgetRows, taxonomyCategories, sort],
  );

  const totals = useMemo(() => {
    const t = { budgeted: 0, spent: 0, priorSpent: 0 };
    for (const node of tree) {
      t.budgeted += node.budgeted;
      t.spent += node.spent;
      t.priorSpent += node.priorSpent;
    }
    return t;
  }, [tree]);

  const groups = useMemo(
    () =>
      groupRows && groupRows.length > 0 ? buildGroupNodes({ tree, groupRows, budgetRows }) : null,
    [tree, groupRows, budgetRows],
  );

  // Expand state for groups + categories lives here so the "Expand all /
  // Collapse all" toggle can flip everything at once. Keys are group ids
  // and category ids (no collision — different id spaces). Groups default
  // to expanded; categories stay collapsed until the user opens them.
  const expandableGroupIds = useMemo(
    () => (groups ?? []).filter((g) => g.children.length > 0).map((g) => g.id),
    [groups],
  );
  const expandableCategoryIds = useMemo(() => {
    const ids: string[] = [];
    if (groups) {
      for (const g of groups) {
        for (const node of g.children) if (node.children.length > 0) ids.push(node.id);
      }
    } else {
      for (const node of tree) if (node.children.length > 0) ids.push(node.id);
    }
    return ids;
  }, [groups, tree]);

  const [expandedById, setExpandedById] = useState<Record<string, boolean>>({});
  // Re-seed when the set of expandable ids changes (groups default open,
  // categories closed); preserve any user toggles. Done during render per
  // React docs guidance to avoid an extra render from useEffect.
  const expandableIdsKey = expandableGroupIds.join(",") + "|" + expandableCategoryIds.join(",");
  const [lastIdsKey, setLastIdsKey] = useState(expandableIdsKey);
  if (expandableIdsKey !== lastIdsKey) {
    setLastIdsKey(expandableIdsKey);
    setExpandedById((prev) => {
      const next: Record<string, boolean> = {};
      for (const id of expandableGroupIds) next[id] = prev[id] ?? true;
      for (const id of expandableCategoryIds) next[id] = prev[id] ?? false;
      return next;
    });
  }

  const hasExpandable = expandableGroupIds.length + expandableCategoryIds.length > 0;
  const allExpanded =
    hasExpandable &&
    expandableGroupIds.every((id) => expandedById[id]) &&
    expandableCategoryIds.every((id) => expandedById[id]);
  const toggleAll = () => {
    const target = !allExpanded;
    const next: Record<string, boolean> = {};
    for (const id of expandableGroupIds) next[id] = target;
    for (const id of expandableCategoryIds) next[id] = target;
    setExpandedById(next);
  };
  const setRowExpanded = useCallback(
    (id: string, value: boolean) => setExpandedById((prev) => ({ ...prev, [id]: value })),
    [],
  );

  if (isLoading) {
    return (
      <div className="space-y-2">
        {Array.from({ length: 6 }).map((_, i) => (
          <Skeleton key={i} className="h-9 w-full" />
        ))}
      </div>
    );
  }

  if (tree.length === 0) {
    return (
      <div className="text-muted-foreground py-8 text-center text-sm">
        No categorized spending in this period.
      </div>
    );
  }

  return (
    <div className="overflow-x-auto">
      <table className="text-foreground w-full text-sm">
        <thead>
          <tr className="border-border/60 text-muted-foreground/80 border-b text-[11px] uppercase tracking-wide">
            <th className="px-3 py-2 text-left font-medium">
              <div className="flex items-center gap-3">
                <span>Category</span>
                {hasExpandable && (
                  <button
                    type="button"
                    onClick={toggleAll}
                    aria-pressed={allExpanded}
                    className="text-muted-foreground hover:text-foreground hover:bg-muted/60 -my-1 inline-flex items-center gap-1 rounded-md px-1.5 py-0.5 text-[10px] font-medium normal-case tracking-normal transition-colors"
                    title={allExpanded ? "Collapse all rows" : "Expand all rows"}
                  >
                    <Icons.ChevronsUpDown
                      className={cn("h-3 w-3 transition-transform", allExpanded && "rotate-180")}
                      aria-hidden
                    />
                    {allExpanded ? "Collapse all" : "Expand all"}
                  </button>
                )}
              </div>
            </th>
            <th className="px-3 py-2 text-right font-medium">Spent / Budget</th>
            <th className="px-3 py-2 text-left font-medium">Progress</th>
            <th className="px-3 py-2 text-right font-medium">Δ vs prior</th>
          </tr>
        </thead>
        <tbody>
          {groups
            ? groups.map((group) => (
                <GroupRow
                  key={group.id}
                  group={group}
                  totalSpent={totals.spent}
                  currency={currency}
                  onCategoryClick={onCategoryClick}
                  expanded={!!expandedById[group.id]}
                  onToggle={() => setRowExpanded(group.id, !expandedById[group.id])}
                  expandedById={expandedById}
                  onChildToggle={setRowExpanded}
                />
              ))
            : tree.map((node) => (
                <ParentRow
                  key={node.id}
                  node={node}
                  currency={currency}
                  onCategoryClick={onCategoryClick}
                  expanded={!!expandedById[node.id]}
                  onToggle={() => setRowExpanded(node.id, !expandedById[node.id])}
                />
              ))}
        </tbody>
        <tfoot>
          <tr className="border-border/60 border-t text-sm font-medium">
            <td className="px-3 py-2.5">Total</td>
            <td className="px-3 py-2.5 text-right text-xs tabular-nums">
              <span className="text-foreground font-medium">
                −<PrivacyAmount value={totals.spent} currency={currency} />
              </span>
              {totals.budgeted > 0 && (
                <span className="text-muted-foreground/70 ml-1">
                  / <PrivacyAmount value={totals.budgeted} currency={currency} />
                </span>
              )}
            </td>
            <td className="px-3 py-2.5">
              {totals.budgeted > 0 ? (
                <ProgressBar spent={totals.spent} budget={totals.budgeted} />
              ) : (
                <span className="text-muted-foreground/50 text-xs">No budget set</span>
              )}
            </td>
            <td
              className={cn(
                "px-3 py-2.5 text-right text-xs tabular-nums",
                totals.priorSpent === 0 || totals.spent - totals.priorSpent === 0
                  ? "text-muted-foreground/70"
                  : totals.spent - totals.priorSpent > 0
                    ? "text-destructive"
                    : "text-success",
              )}
            >
              {formatDelta(totals.spent - totals.priorSpent, totals.priorSpent)}
            </td>
          </tr>
        </tfoot>
      </table>
    </div>
  );
}

function ProgressBar({ spent, budget }: { spent: number; budget: number }) {
  if (budget <= 0) return null;
  const pct = (spent / budget) * 100;
  const isOver = pct > 100;
  const isClose = pct >= 85 && !isOver;
  const fillColor = isOver
    ? "var(--destructive)"
    : isClose
      ? "var(--status-warn)"
      : "var(--success)";
  return (
    <div className="flex items-center gap-2">
      <div className="bg-foreground/10 relative h-1.5 min-w-[60px] flex-1 overflow-hidden rounded-full">
        <div
          className="h-full rounded-full transition-all"
          style={{ width: `${Math.min(100, pct)}%`, backgroundColor: fillColor, opacity: 0.65 }}
        />
      </div>
      <span
        className={cn(
          "w-10 shrink-0 text-right text-[11px] tabular-nums",
          isOver ? "text-destructive font-medium" : "text-muted-foreground/80",
        )}
      >
        {pct.toFixed(0)}%
      </span>
    </div>
  );
}

const GroupRow = memo(function GroupRow({
  group,
  totalSpent,
  currency,
  onCategoryClick,
  expanded,
  onToggle,
  expandedById,
  onChildToggle,
}: {
  group: GroupNode;
  totalSpent: number;
  currency: string;
  onCategoryClick?: (categoryId: string) => void;
  expanded: boolean;
  onToggle: () => void;
  expandedById: Record<string, boolean>;
  onChildToggle: (id: string, value: boolean) => void;
}) {
  const hasChildren = group.children.length > 0;
  const delta = group.spent - group.priorSpent;
  const sharePct = totalSpent > 0 ? (group.spent / totalSpent) * 100 : 0;
  const accent = group.color ?? "var(--muted-foreground)";

  return (
    <>
      <tr
        className={cn(
          "border-border/60 bg-muted/20 hover:bg-muted/30 border-b border-t-0",
          hasChildren && "cursor-pointer",
        )}
        onClick={hasChildren ? onToggle : undefined}
      >
        <td className="px-3 py-2.5">
          <div className="flex items-center gap-2">
            <Icons.ChevronRight
              className={cn(
                "text-muted-foreground/70 h-3.5 w-3.5 transition-transform",
                expanded && "rotate-90",
                !hasChildren && "opacity-0",
              )}
            />
            <span
              className="block h-2 w-2 shrink-0 rounded-full"
              style={{ backgroundColor: accent }}
            />
            <span className="text-foreground text-sm font-semibold uppercase tracking-wide">
              {group.name}
            </span>
            <span className="text-muted-foreground/70 text-[11px] font-medium tabular-nums">
              {sharePct.toFixed(1)}%
            </span>
          </div>
        </td>
        <td className="text-foreground px-3 py-2.5 text-right text-xs font-semibold tabular-nums">
          −<PrivacyAmount value={group.spent} currency={currency} />
          {group.budgeted > 0 && (
            <span className="text-muted-foreground/70 ml-1 font-normal">
              / <PrivacyAmount value={group.budgeted} currency={currency} />
            </span>
          )}
        </td>
        <td className="px-3 py-2.5">
          {group.budgeted > 0 ? (
            <ProgressBar spent={group.spent} budget={group.budgeted} />
          ) : (
            <span className="text-muted-foreground/50 text-xs">—</span>
          )}
        </td>
        <td
          className={cn(
            "px-3 py-2.5 text-right text-xs font-medium tabular-nums",
            delta === 0 || group.priorSpent === 0
              ? "text-muted-foreground/70"
              : delta > 0
                ? "text-destructive"
                : "text-success",
          )}
        >
          {formatDelta(delta, group.priorSpent)}
        </td>
      </tr>
      {expanded &&
        group.children.map((node) => (
          <ParentRow
            key={node.id}
            node={node}
            currency={currency}
            onCategoryClick={onCategoryClick}
            indented
            expanded={!!expandedById[node.id]}
            onToggle={() => onChildToggle(node.id, !expandedById[node.id])}
          />
        ))}
    </>
  );
});

const ParentRow = memo(function ParentRow({
  node,
  currency,
  onCategoryClick,
  indented = false,
  expanded,
  onToggle,
}: {
  node: NodeRow;
  currency: string;
  onCategoryClick?: (categoryId: string) => void;
  /** Nested under a group — adds left padding so the category column aligns. */
  indented?: boolean;
  expanded: boolean;
  onToggle: () => void;
}) {
  const hasChildren = node.children.length > 0;
  const delta = node.spent - node.priorSpent;
  const accent = node.color ?? "var(--muted-foreground)";
  const tintBg = node.color ? `${node.color}1F` : "var(--muted)";
  const clickable = !!onCategoryClick;

  return (
    <>
      <tr
        className={cn(
          "border-border/40 hover:bg-muted/30 group border-b",
          clickable && "cursor-pointer",
        )}
        onClick={clickable ? () => onCategoryClick?.(node.id) : undefined}
      >
        <td className={cn("px-3 py-2.5", indented && "pl-8")}>
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={(e) => {
                e.stopPropagation();
                onToggle();
              }}
              className="text-muted-foreground/70 hover:text-foreground -m-1 flex h-5 w-5 items-center justify-center rounded p-1"
              aria-expanded={expanded}
              aria-label={hasChildren ? "Toggle subcategories" : undefined}
              disabled={!hasChildren}
            >
              <Icons.ChevronRight
                className={cn(
                  "h-3.5 w-3.5 transition-transform",
                  expanded && "rotate-90",
                  !hasChildren && "opacity-0",
                )}
              />
            </button>
            <span
              className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full"
              style={{ backgroundColor: tintBg, color: accent }}
            >
              <CategoryIcon icon={node.icon} fallback={node.name} className="h-3.5 w-3.5" />
            </span>
            <span className="text-foreground text-sm font-medium">{node.name}</span>
          </div>
        </td>
        <td className="text-foreground/90 px-3 py-2.5 text-right text-xs tabular-nums">
          <span className="text-foreground font-medium">
            −<PrivacyAmount value={node.spent} currency={currency} />
          </span>
          {node.budgeted > 0 && (
            <span className="text-muted-foreground/70 ml-1">
              / <PrivacyAmount value={node.budgeted} currency={currency} />
            </span>
          )}
        </td>
        <td className="px-3 py-2.5">
          {node.budgeted > 0 ? (
            <ProgressBar spent={node.spent} budget={node.budgeted} />
          ) : (
            <span className="text-muted-foreground/50 text-xs">No budget set</span>
          )}
        </td>
        <td
          className={cn(
            "px-3 py-2.5 text-right text-xs tabular-nums",
            delta === 0 || node.priorSpent === 0
              ? "text-muted-foreground/70"
              : delta > 0
                ? "text-destructive"
                : "text-success",
          )}
        >
          {formatDelta(delta, node.priorSpent)}
        </td>
      </tr>
      {expanded &&
        node.children.map((child) => (
          <ChildRow
            key={child.id}
            node={child}
            currency={currency}
            parentColor={accent}
            onCategoryClick={onCategoryClick}
            indented={indented}
          />
        ))}
    </>
  );
});

const ChildRow = memo(function ChildRow({
  node,
  currency,
  parentColor,
  onCategoryClick,
  indented = false,
}: {
  node: NodeRow;
  currency: string;
  parentColor: string;
  onCategoryClick?: (categoryId: string) => void;
  indented?: boolean;
}) {
  const delta = node.spent - node.priorSpent;
  const clickable = !!onCategoryClick;
  return (
    <tr
      className={cn(
        "border-border/30 hover:bg-muted/20 border-b text-[13px]",
        clickable && "cursor-pointer",
      )}
      onClick={clickable ? () => onCategoryClick?.(node.id) : undefined}
    >
      <td className={cn("text-muted-foreground/90 px-3 py-1.5 pl-9", indented && "pl-14")}>
        <div className="flex items-center gap-2">
          <span
            className="h-1 w-1 shrink-0 rounded-full"
            style={{ backgroundColor: parentColor, opacity: 0.6 }}
          />
          <span>{node.name}</span>
        </div>
      </td>
      <td className="text-muted-foreground/90 px-3 py-1.5 text-right text-xs tabular-nums">
        −<PrivacyAmount value={node.spent} currency={currency} />
      </td>
      <td className="px-3 py-1.5"></td>
      <td
        className={cn(
          "px-3 py-1.5 text-right text-xs tabular-nums",
          delta === 0 || node.priorSpent === 0
            ? "text-muted-foreground/60"
            : delta > 0
              ? "text-destructive"
              : "text-success",
        )}
      >
        {formatDelta(delta, node.priorSpent)}
      </td>
    </tr>
  );
});
