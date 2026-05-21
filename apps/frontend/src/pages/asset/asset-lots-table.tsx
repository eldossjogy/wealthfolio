import { useState } from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@wealthfolio/ui/components/ui/table";
import type { AssetLotView } from "@/lib/types";
import { formatAmount } from "@wealthfolio/ui";
import { formatDate, formatQuantity } from "@/lib/utils";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Card, CardContent } from "@wealthfolio/ui/components/ui/card";
import { GainAmount } from "@wealthfolio/ui";
import { GainPercent } from "@wealthfolio/ui";
import { Icons } from "@wealthfolio/ui/components/ui/icons";

interface AssetLotsTableProps {
  lots: AssetLotView[];
  currency: string;
  marketPrice: number;
  contractMultiplier?: number;
}

export const AssetLotsTable = ({
  lots,
  currency,
  marketPrice,
  contractMultiplier = 1,
}: AssetLotsTableProps) => {
  if (!lots || lots.length === 0) {
    return null;
  }

  const groups = groupLotsByAccount(lots);
  const isMultiAccount = groups.length > 1;

  return (
    <Card className="mt-4">
      <CardContent className="p-0">
        {groups.map((group) => (
          <AccountLotGroup
            key={group.accountId}
            accountName={group.accountName}
            lots={group.lots}
            currency={currency}
            marketPrice={marketPrice}
            contractMultiplier={contractMultiplier}
            collapsible={isMultiAccount}
          />
        ))}
      </CardContent>
    </Card>
  );
};

interface AccountLotGroupData {
  accountId: string;
  accountName: string;
  lots: AssetLotView[];
}

function groupLotsByAccount(lots: AssetLotView[]): AccountLotGroupData[] {
  const byAccount = new Map<string, AccountLotGroupData>();

  for (const lot of lots) {
    const group = byAccount.get(lot.accountId) ?? {
      accountId: lot.accountId,
      accountName: lot.accountName || lot.accountId,
      lots: [],
    };
    group.lots.push(lot);
    byAccount.set(lot.accountId, group);
  }

  return [...byAccount.values()]
    .map((group) => ({
      ...group,
      lots: [...group.lots].sort(compareLots),
    }))
    .sort((a, b) => a.accountName.localeCompare(b.accountName));
}

function compareLots(a: AssetLotView, b: AssetLotView) {
  const aRank = a.isClosed ? 2 : a.source === "SNAPSHOT_POSITION" ? 1 : 0;
  const bRank = b.isClosed ? 2 : b.source === "SNAPSHOT_POSITION" ? 1 : 0;
  if (aRank !== bRank) return aRank - bRank;

  const aDate = new Date(a.acquisitionDate ?? a.snapshotDate ?? "").getTime();
  const bDate = new Date(b.acquisitionDate ?? b.snapshotDate ?? "").getTime();
  return aDate - bDate || a.id.localeCompare(b.id);
}

function AccountLotGroup({
  accountName,
  lots,
  currency,
  marketPrice,
  contractMultiplier,
  collapsible,
}: {
  accountName: string;
  lots: AssetLotView[];
  currency: string;
  marketPrice: number;
  contractMultiplier: number;
  collapsible: boolean;
}) {
  const [expanded, setExpanded] = useState(true);
  const summary = getGroupSummary(lots);

  return (
    <div>
      {collapsible && (
        <button
          type="button"
          onClick={() => setExpanded((value) => !value)}
          className="hover:bg-muted flex w-full items-center gap-2 px-4 py-2 text-sm font-medium"
        >
          {expanded ? (
            <Icons.ChevronDown className="h-4 w-4" />
          ) : (
            <Icons.ChevronRight className="h-4 w-4" />
          )}
          <span>{accountName}</span>
          <span className="text-muted-foreground ml-auto text-xs">{summary}</span>
        </button>
      )}

      {(!collapsible || expanded) && (
        <>
          <div className="hidden overflow-x-auto md:block">
            <Table>
              <TableHeader className="bg-muted">
                <TableRow>
                  <TableHead>Source</TableHead>
                  <TableHead className="w-[160px]">Date</TableHead>
                  <TableHead className="text-right">Original Qty</TableHead>
                  <TableHead className="text-right">Remaining</TableHead>
                  <TableHead className="text-right">Unit Cost</TableHead>
                  <TableHead className="text-right">Fees</TableHead>
                  <TableHead className="text-right">Cost Basis</TableHead>
                  <TableHead className="text-right">Market Value</TableHead>
                  <TableHead className="text-right">Gain/Loss</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {lots.map((lot) => (
                  <AssetLotTableRow
                    key={lot.id}
                    lot={lot}
                    currency={currency}
                    marketPrice={marketPrice}
                    contractMultiplier={contractMultiplier}
                  />
                ))}
              </TableBody>
            </Table>
          </div>

          <div className="divide-y md:hidden">
            {lots.map((lot) => (
              <AssetLotMobileRow
                key={lot.id}
                lot={lot}
                accountName={collapsible ? undefined : accountName}
                currency={currency}
                marketPrice={marketPrice}
                contractMultiplier={contractMultiplier}
              />
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function AssetLotTableRow({
  lot,
  currency,
  marketPrice,
  contractMultiplier,
}: {
  lot: AssetLotView;
  currency: string;
  marketPrice: number;
  contractMultiplier: number;
}) {
  const values = getLotDisplayValues(lot, marketPrice, contractMultiplier);

  return (
    <TableRow className={lot.isClosed ? "opacity-60" : undefined}>
      <TableCell>
        <LotBadges lot={lot} />
      </TableCell>
      <TableCell className="font-medium">
        {formatLotDate(lot)}
        {lot.isClosed && lot.closeDate && (
          <div className="text-muted-foreground text-xs">Closed {formatDate(lot.closeDate)}</div>
        )}
      </TableCell>
      <TableCell className="text-right">
        {lot.source === "SNAPSHOT_POSITION" ? "-" : formatQuantity(lot.originalQuantity)}
      </TableCell>
      <TableCell className="text-right">
        {formatQuantity(values.remainingQuantity)}
        {values.showEffectiveQuantity && (
          <div className="text-muted-foreground text-xs">
            Effective {formatQuantity(values.effectiveQuantity)}
          </div>
        )}
      </TableCell>
      <TableCell className="text-right">
        {formatAmount(lot.unitCost, currency)}
        {values.showAdjustedUnitCost && (
          <div className="text-muted-foreground text-xs">
            Adj. {formatAmount(lot.unitCost / lot.splitRatio, currency)}
          </div>
        )}
      </TableCell>
      <TableCell className="text-right">
        {lot.source === "SNAPSHOT_POSITION" ? "-" : formatAmount(lot.fees, currency)}
      </TableCell>
      <TableCell className="text-right">{formatAmount(lot.costBasis, currency)}</TableCell>
      <TableCell className="text-right">
        {values.isValuable ? formatAmount(values.marketValue, currency) : "-"}
      </TableCell>
      <TableCell className="text-right">
        {values.isValuable ? (
          <div className="flex flex-row items-center justify-end space-x-2">
            <GainAmount value={values.gainLossAmount} currency={currency} displayCurrency={false} />
            <GainPercent value={values.gainLossPercent} variant="badge" />
          </div>
        ) : (
          "-"
        )}
      </TableCell>
    </TableRow>
  );
}

function AssetLotMobileRow({
  lot,
  accountName,
  currency,
  marketPrice,
  contractMultiplier,
}: {
  lot: AssetLotView;
  accountName?: string;
  currency: string;
  marketPrice: number;
  contractMultiplier: number;
}) {
  const values = getLotDisplayValues(lot, marketPrice, contractMultiplier);

  return (
    <div className={`space-y-2 p-4 ${lot.isClosed ? "opacity-60" : ""}`}>
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 space-y-1">
          <div className="flex flex-wrap items-center gap-2">
            <LotBadges lot={lot} />
            <span className="text-sm font-medium">{formatLotDate(lot)}</span>
          </div>
          {accountName && <div className="text-muted-foreground text-xs">{accountName}</div>}
          {lot.isClosed && lot.closeDate && (
            <div className="text-muted-foreground text-xs">Closed {formatDate(lot.closeDate)}</div>
          )}
        </div>
        {values.isValuable && (
          <div className="flex shrink-0 items-center space-x-2">
            <GainAmount value={values.gainLossAmount} currency={currency} displayCurrency={false} />
            <GainPercent value={values.gainLossPercent} variant="badge" />
          </div>
        )}
      </div>

      <div className="text-muted-foreground grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
        {lot.source !== "SNAPSHOT_POSITION" && (
          <>
            <span>Original Qty</span>
            <span className="text-foreground text-right">
              {formatQuantity(lot.originalQuantity)}
            </span>
          </>
        )}
        <span>Remaining</span>
        <span className="text-foreground text-right">
          {formatQuantity(values.remainingQuantity)}
          {values.showEffectiveQuantity && (
            <span className="text-muted-foreground block text-xs">
              Effective {formatQuantity(values.effectiveQuantity)}
            </span>
          )}
        </span>
        <span>Unit Cost</span>
        <span className="text-foreground text-right">
          {formatAmount(lot.unitCost, currency)}
          {values.showAdjustedUnitCost && (
            <span className="text-muted-foreground block text-xs">
              Adj. {formatAmount(lot.unitCost / lot.splitRatio, currency)}
            </span>
          )}
        </span>
        {lot.source !== "SNAPSHOT_POSITION" && (
          <>
            <span>Fees</span>
            <span className="text-foreground text-right">{formatAmount(lot.fees, currency)}</span>
          </>
        )}
        <span>Cost Basis</span>
        <span className="text-foreground text-right">{formatAmount(lot.costBasis, currency)}</span>
        {values.isValuable && (
          <>
            <span>Market Value</span>
            <span className="text-foreground text-right">
              {formatAmount(values.marketValue, currency)}
            </span>
          </>
        )}
      </div>
    </div>
  );
}

function LotBadges({ lot }: { lot: AssetLotView }) {
  const isSnapshot = lot.source === "SNAPSHOT_POSITION";
  const hasSplit = !isSnapshot && lot.splitRatio !== 1;

  return (
    <div className="flex flex-wrap items-center gap-1">
      <Badge variant={isSnapshot ? "secondary" : "outline"} className="text-xs">
        {isSnapshot ? "Snapshot" : "Transaction"}
      </Badge>
      {!isSnapshot && (
        <Badge variant={lot.isClosed ? "secondary" : "outline"} className="text-xs">
          {lot.isClosed ? "Closed" : "Open"}
        </Badge>
      )}
      {hasSplit && (
        <Badge
          variant="outline"
          className="text-xs"
          title={`As-acquired units. Effective quantity = remaining quantity x ${lot.splitRatio}.`}
        >
          {`${lot.splitRatio}:1 split`}
        </Badge>
      )}
    </div>
  );
}

function getGroupSummary(lots: AssetLotView[]) {
  const snapshots = lots.filter((lot) => lot.source === "SNAPSHOT_POSITION").length;
  const open = lots.filter((lot) => lot.source !== "SNAPSHOT_POSITION" && !lot.isClosed).length;
  const closed = lots.filter((lot) => lot.source !== "SNAPSHOT_POSITION" && lot.isClosed).length;
  const parts = [];

  if (open > 0) parts.push(`${open} open`);
  if (closed > 0) parts.push(`${closed} closed`);
  if (snapshots > 0) parts.push(`${snapshots} snapshot`);

  return parts.join(", ");
}

function formatLotDate(lot: AssetLotView) {
  const date = lot.acquisitionDate ?? lot.snapshotDate;
  return date ? formatDate(date) : "-";
}

function getLotDisplayValues(lot: AssetLotView, marketPrice: number, contractMultiplier: number) {
  const isSnapshot = lot.source === "SNAPSHOT_POSITION";
  const splitRatio = lot.splitRatio || 1;
  const rowContractMultiplier = lot.contractMultiplier || contractMultiplier || 1;
  const remainingQuantity = isSnapshot ? lot.quantity : lot.remainingQuantity;
  const effectiveQuantity = isSnapshot ? lot.quantity : remainingQuantity * splitRatio;
  const isValuable = !lot.isClosed;
  const marketValue = effectiveQuantity * marketPrice * rowContractMultiplier;
  const gainLossAmount = marketValue - lot.costBasis;
  const gainLossPercent = lot.costBasis !== 0 ? gainLossAmount / lot.costBasis : 0;

  return {
    effectiveQuantity,
    remainingQuantity,
    marketValue,
    gainLossAmount,
    gainLossPercent,
    isValuable,
    showEffectiveQuantity: !isSnapshot && splitRatio !== 1,
    showAdjustedUnitCost: !isSnapshot && splitRatio !== 1,
  };
}

export default AssetLotsTable;
