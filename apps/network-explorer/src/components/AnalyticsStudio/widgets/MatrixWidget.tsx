import { useMemo, useEffect, useRef } from 'react';
import type { Widget } from '../../../analytics/registry/types';
import { useDashboardStore } from '../../../store/dashboardStore';
import { useBenchmarkStore } from '../../../store/benchmarkStore';
import WidgetShell from './WidgetShell';

//-------------------------------------
// Matrix Widget
//
// Performs client-side pivoting of standard query results (Row × Col → Val).
// Participates in cross-highlighting:
//   - Filtered: only renders matching items when an external highlight matches rows/cols.
//   - Emitter: clicking a row header or cell triggers dashboard highlight.
//-------------------------------------

interface Props {
  widget: Widget;
}

/** Helper to check if a matrix dimension matches the highlight column. */
function columnMatches(dimCol: string | null, highlightCol: string | null): boolean {
  if (!dimCol || !highlightCol) return false;
  if (dimCol === highlightCol) return true;
  if (dimCol.includes('.') && dimCol.split('.')[1] === highlightCol) return true;
  if (highlightCol.includes('.') && highlightCol.split('.')[1] === dimCol) return true;
  return false;
}

export default function MatrixWidget({ widget }: Props) {
  const highlightContext = useDashboardStore((s) => s.highlightContext);
  const setHighlight = useDashboardStore((s) => s.setHighlight);
  const clearHighlight = useDashboardStore((s) => s.clearHighlight);

  const rawRows = useMemo(() => widget.result?.rows ?? [], [widget.result?.rows]);
  const renderStartRef = useRef<number | null>(null);

  // Measure initial render time
  useEffect(() => {
    if (!widget.isLoading && widget.result) {
      renderStartRef.current = performance.now();
    }
  }, [widget.result, widget.isLoading]);

  useEffect(() => {
    if (renderStartRef.current != null) {
      const ms = performance.now() - renderStartRef.current;
      useBenchmarkStore.getState().addEvent({
        eventType: 'matrix-render',
        durationMs: ms,
        widgetId: widget.id,
        rowsReturned: widget.result?.rowCount,
      });
      renderStartRef.current = null;
    }
  }, [rawRows, widget.id, widget.result]);

  // Measure highlight propagation rendering latency
  useEffect(() => {
    if (highlightContext.appliedAt != null) {
      const ms = performance.now() - highlightContext.appliedAt;
      useBenchmarkStore.getState().addEvent({
        eventType: 'cross-highlight',
        durationMs: ms,
        widgetId: widget.id,
        metadata: { column: highlightContext.column, value: highlightContext.value }
      });
    }
  }, [highlightContext, widget.id]);

  // 1. Filter dataset client-side when chart highlight matches matrix rows or columns
  const filteredRows = useMemo(() => {
    if (!highlightContext.value || !highlightContext.column) return rawRows;

    const rowMatch = columnMatches(widget.matrixRowCol || null, highlightContext.column);
    const colMatch = columnMatches(widget.matrixColCol || null, highlightContext.column);

    if (rowMatch) {
      return rawRows.filter((r) => String(r.row ?? '') === highlightContext.value);
    }
    if (colMatch) {
      return rawRows.filter((r) => String(r.col ?? '') === highlightContext.value);
    }
    return rawRows;
  }, [rawRows, highlightContext, widget.matrixRowCol, widget.matrixColCol]);

  // 2. Extract dimensions and pivot values
  const { rowValues, colValues, cellMap, rowTotals, colTotals, grandTotal } = useMemo(() => {
    const rowsSet = new Set<string>();
    const colsSet = new Set<string>();
    const cells: Record<string, Record<string, number>> = {};

    const rTotals: Record<string, number> = {};
    const cTotals: Record<string, number> = {};
    let gTotal = 0;

    for (const r of filteredRows) {
      const rVal = String(r.row ?? 'null');
      const cVal = String(r.col ?? 'null');
      const val = Number(r.value ?? 0);

      rowsSet.add(rVal);
      colsSet.add(cVal);

      if (!cells[rVal]) cells[rVal] = {};
      cells[rVal][cVal] = val;

      rTotals[rVal] = (rTotals[rVal] || 0) + val;
      cTotals[cVal] = (cTotals[cVal] || 0) + val;
      gTotal += val;
    }

    return {
      rowValues: Array.from(rowsSet).sort(),
      colValues: Array.from(colsSet).sort(),
      cellMap: cells,
      rowTotals: rTotals,
      colTotals: cTotals,
      grandTotal: gTotal,
    };
  }, [filteredRows]);

  // Handle header click to set dashboard highlight
  const handleRowClick = (rowVal: string) => {
    if (widget.matrixRowCol) {
      if (highlightContext.column === widget.matrixRowCol && highlightContext.value === rowVal) {
        clearHighlight();
      } else {
        setHighlight({
          column: widget.matrixRowCol,
          value: rowVal,
        });
      }
    }
  };

  const isRowHighlighted = (rowVal: string) => {
    return (
      highlightContext.value === rowVal &&
      columnMatches(widget.matrixRowCol || null, highlightContext.column)
    );
  };

  return (
    <WidgetShell widget={widget}>
      {rawRows.length === 0 ? (
        <p className="widget-no-data">No data available for matrix.</p>
      ) : (
        <div className="matrix-widget-scroll" style={{ height: '100%', width: '100%', overflow: 'auto' }}>
          <table className="matrix-widget-table">
            <thead>
              <tr>
                <th className="matrix-th-corner">
                  {widget.matrixRowCol?.split('.').pop()} \ {widget.matrixColCol?.split('.').pop()}
                </th>
                {colValues.map((col) => (
                  <th key={col} className="matrix-th-col">
                    {col}
                  </th>
                ))}
                <th className="matrix-th-total">Total</th>
              </tr>
            </thead>
            <tbody>
              {rowValues.map((row) => {
                const highlighted = isRowHighlighted(row);
                return (
                  <tr
                    key={row}
                    className={`matrix-row${highlighted ? ' matrix-row--highlighted' : ''}`}
                  >
                    <td
                      className="matrix-td-row-header"
                      onClick={() => handleRowClick(row)}
                      title={`Click to highlight row: ${row}`}
                    >
                      {row}
                    </td>
                    {colValues.map((col) => {
                      const val = cellMap[row]?.[col];
                      return (
                        <td
                          key={col}
                          className="matrix-td-cell"
                          onClick={() => handleRowClick(row)}
                          title={`Click to highlight: ${row}`}
                        >
                          {val != null ? val.toLocaleString() : '—'}
                        </td>
                      );
                    })}
                    <td className="matrix-td-row-total">
                      {rowTotals[row]?.toLocaleString() ?? 0}
                    </td>
                  </tr>
                );
              })}
              {/* Totals row */}
              <tr className="matrix-totals-row">
                <td className="matrix-td-row-header font-semibold">Total</td>
                {colValues.map((col) => (
                  <td key={col} className="matrix-td-col-total">
                    {colTotals[col]?.toLocaleString() ?? 0}
                  </td>
                ))}
                <td className="matrix-td-grand-total">
                  {grandTotal.toLocaleString()}
                </td>
              </tr>
            </tbody>
          </table>
        </div>
      )}
    </WidgetShell>
  );
}
