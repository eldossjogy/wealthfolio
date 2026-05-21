import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@wealthfolio/ui/components/ui/table";
import type { AssetLotViewRow } from "@/lib/types";
import { formatAmount } from "@wealthfolio/ui";
import { formatDate, formatQuantity } from "@/lib/utils";
import { Badge } from "@wealthfolio/ui/components/ui/badge";
import { Card, CardContent } from "@wealthfolio/ui/components/ui/card";
import { GainAmount } from "@wealthfolio/ui";
import { GainPercent } from "@wealthfolio/ui";

interface AssetLotsTableProps {
  lots: AssetLotViewRow[];
  currency: string;
  marketPrice: number;
}

export const AssetLotsTable = ({ lots, currency, marketPrice }: AssetLotsTableProps) => {
  if (!lots || lots.length === 0) {
    return null;
  }

  const sortedLots = [...lots].sort(
    (a, b) =>
      new Date(a.acquisitionDate ?? a.snapshotDate ?? "").getTime() -
      new Date(b.acquisitionDate ?? b.snapshotDate ?? "").getTime(),
  );

  const getDateLabel = (lot: AssetLotViewRow) => {
    const date = lot.acquisitionDate ?? lot.snapshotDate;
    return date ? formatDate(date) : "-";
  };

  const getSourceLabel = (lot: AssetLotViewRow) =>
    lot.source === "SNAPSHOT_POSITION" ? "Snapshot" : "Transaction";

  return (
    <Card className="mt-4">
      <CardContent className="p-0">
        {/* Desktop table */}
        <div className="hidden overflow-x-auto md:block">
          <Table>
            <TableHeader className="bg-muted">
              <TableRow>
                <TableHead className="w-[160px]">Date</TableHead>
                <TableHead>Source</TableHead>
                <TableHead className="text-right">Quantity</TableHead>
                <TableHead className="text-right">Unit Cost</TableHead>
                <TableHead className="text-right">Fees</TableHead>
                <TableHead className="text-right">Cost Basis</TableHead>
                <TableHead className="text-right">Market Value</TableHead>
                <TableHead className="text-right">Gain/Loss</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {sortedLots.map((lot) => {
                const marketValue = lot.quantity * marketPrice;
                const gainLossAmount = marketValue - lot.costBasis;
                const gainLossPercent = lot.costBasis !== 0 ? gainLossAmount / lot.costBasis : 0;

                return (
                  <TableRow key={lot.id}>
                    <TableCell className="font-medium">{getDateLabel(lot)}</TableCell>
                    <TableCell>
                      <Badge variant="secondary">{getSourceLabel(lot)}</Badge>
                    </TableCell>
                    <TableCell className="text-right">{formatQuantity(lot.quantity)}</TableCell>
                    <TableCell className="text-right">
                      {formatAmount(lot.unitCost, currency)}
                    </TableCell>
                    <TableCell className="text-right">{formatAmount(lot.fees, currency)}</TableCell>
                    <TableCell className="text-right">
                      {formatAmount(lot.costBasis, currency)}
                    </TableCell>
                    <TableCell className="text-right">
                      {formatAmount(marketValue, currency)}
                    </TableCell>
                    <TableCell className="text-right">
                      <div className="flex flex-row items-center justify-end space-x-2">
                        <GainAmount
                          value={gainLossAmount}
                          currency={currency}
                          displayCurrency={false}
                        />
                        <GainPercent value={gainLossPercent} variant="badge" />
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </div>

        {/* Mobile card list */}
        <div className="divide-y md:hidden">
          {sortedLots.map((lot) => {
            const marketValue = lot.quantity * marketPrice;
            const gainLossAmount = marketValue - lot.costBasis;
            const gainLossPercent = lot.costBasis !== 0 ? gainLossAmount / lot.costBasis : 0;

            return (
              <div key={lot.id} className="space-y-2 p-4">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="text-sm font-medium">{getDateLabel(lot)}</span>
                    <Badge variant="secondary">{getSourceLabel(lot)}</Badge>
                  </div>
                  <div className="flex items-center space-x-2">
                    <GainAmount
                      value={gainLossAmount}
                      currency={currency}
                      displayCurrency={false}
                    />
                    <GainPercent value={gainLossPercent} variant="badge" />
                  </div>
                </div>
                <div className="text-muted-foreground grid grid-cols-2 gap-x-4 gap-y-1 text-sm">
                  <span>Quantity</span>
                  <span className="text-foreground text-right">{formatQuantity(lot.quantity)}</span>
                  <span>Unit Cost</span>
                  <span className="text-foreground text-right">
                    {formatAmount(lot.unitCost, currency)}
                  </span>
                  <span>Fees</span>
                  <span className="text-foreground text-right">
                    {formatAmount(lot.fees, currency)}
                  </span>
                  <span>Cost Basis</span>
                  <span className="text-foreground text-right">
                    {formatAmount(lot.costBasis, currency)}
                  </span>
                  <span>Market Value</span>
                  <span className="text-foreground text-right">
                    {formatAmount(marketValue, currency)}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      </CardContent>
    </Card>
  );
};

export default AssetLotsTable;
