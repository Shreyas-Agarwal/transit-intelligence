import { useState, useMemo, useEffect, useRef } from 'react';
import type { Widget } from '../../../analytics/registry/types';
import { useDashboardStore } from '../../../store/dashboardStore';
import { useBenchmarkStore } from '../../../store/benchmarkStore';
import WidgetShell from './WidgetShell';

//-------------------------------------
// Table Widget
//
// Renders query results as a scrollable, sortable table.
// Participates in cross-highlighting:
//   - Filtered: only renders rows matching external highlights (charts).
//   - Emitter: clicking a cell sets dashboard highlight, updating other widgets.
//-------------------------------------

interface Props {
  widget: Widget;
}

export default function TableWidget({ widget }: Props) {
  const [sortCol, setSortCol] = useState<string | null>(null);
  const [sortAsc, setSortAsc] = useState(true);

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
        eventType: 'table-render',
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

  // 1. Client-side filtering: filter table rows when a chart highlight matches one of our columns
  const filteredRows = useMemo(() => {
    if (!highlightContext.value || !highlightContext.column || rawRows.length === 0) {
      return rawRows;
    }

    const firstRow = rawRows[0];
    const matchingCol = Object.keys(firstRow).find((col) => {
      let configName = col;
      if (widget.tableColumns) {
        const conf = widget.tableColumns.find((c) => c.alias === col || c.name === col);
        if (conf) configName = conf.name;
      }
      return (
        configName === highlightContext.column ||
        configName.split('.').pop() === highlightContext.column ||
        highlightContext.column?.split('.').pop() === configName
      );
    });

    if (!matchingCol) return rawRows;

    return rawRows.filter((r) => String(r[matchingCol] ?? '') === highlightContext.value);
  }, [rawRows, highlightContext, widget.tableColumns]);

  // 2. Derive column headers
  const columns = useMemo(() => {
    if (widget.tableColumns && widget.tableColumns.length > 0) {
      return widget.tableColumns.map((c) => c.alias || c.name);
    }
    return rawRows.length > 0 ? Object.keys(rawRows[0]) : [];
  }, [rawRows, widget.tableColumns]);

  // 3. Client-side sorting
  const sortedRows = useMemo(() => {
    if (!sortCol) return filteredRows;

    return [...filteredRows].sort((a, b) => {
      const av = a[sortCol];
      const bv = b[sortCol];
      if (av == null && bv == null) return 0;
      if (av == null) return 1;
      if (bv == null) return -1;

      const numA = Number(av);
      const numB = Number(bv);
      if (!isNaN(numA) && !isNaN(numB)) {
        return sortAsc ? numA - numB : numB - numA;
      }
      return sortAsc
        ? String(av).localeCompare(String(bv))
        : String(bv).localeCompare(String(av));
    });
  }, [filteredRows, sortCol, sortAsc]);

  const handleSort = (col: string) => {
    if (sortCol === col) {
      setSortAsc((prev) => !prev);
    } else {
      setSortCol(col);
      setSortAsc(true);
    }
  };

  // Click handler to set dashboard-wide highlight
  const handleCellClick = (colName: string, value: unknown) => {
    if (value == null) return;

    let columnKey = colName;
    if (widget.tableColumns) {
      const colConfig = widget.tableColumns.find((c) => c.alias === colName || c.name === colName);
      if (colConfig) {
        // Only allow highlighting dimension values, not aggregated measure calculations
        if (colConfig.aggregation !== 'none') return;
        columnKey = colConfig.name;
      }
    }

    if (highlightContext.column === columnKey && highlightContext.value === String(value)) {
      clearHighlight();
    } else {
      setHighlight({
        column: columnKey,
        value: String(value),
      });
    }
  };

  // Check if a row matches active highlight context
  const isRowHighlighted = (row: Record<string, unknown>) => {
    if (!highlightContext.value || !highlightContext.column) return false;

    return Object.entries(row).some(([colName, val]) => {
      let configName = colName;
      if (widget.tableColumns) {
        const conf = widget.tableColumns.find((c) => c.alias === colName || c.name === colName);
        if (conf) {
          if (conf.aggregation !== 'none') return false;
          configName = conf.name;
        }
      }
      return (
        String(val) === highlightContext.value &&
        (configName === highlightContext.column ||
          configName.split('.').pop() === highlightContext.column ||
          highlightContext.column?.split('.').pop() === configName)
      );
    });
  };

  return (
    <WidgetShell widget={widget}>
      {rawRows.length === 0 ? (
        <p className="widget-no-data">No rows returned.</p>
      ) : (
        <div className="table-widget-scroll" style={{ height: '100%', width: '100%', overflow: 'auto' }}>
          <table className="table-widget-table">
            <thead>
              <tr>
                {columns.map((col) => (
                  <th
                    key={col}
                    className={`table-widget-th${sortCol === col ? ' sort-active' : ''}`}
                    onClick={() => handleSort(col)}
                    title={`Sort by ${col}`}
                  >
                    {col.split('.').pop()}
                    {sortCol === col && (
                      <span className="sort-indicator">{sortAsc ? ' ↑' : ' ↓'}</span>
                    )}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {sortedRows.map((row, i) => {
                const highlighted = isRowHighlighted(row);
                return (
                  <tr
                    key={i}
                    className={`table-widget-row${highlighted ? ' table-widget-row--highlighted' : ''}`}
                  >
                    {columns.map((col) => (
                      <td
                        key={col}
                        className="table-widget-td cursor-pointer hover:bg-indigo-900/10"
                        onClick={() => handleCellClick(col, row[col])}
                        title={row[col] != null ? `Click to highlight "${row[col]}"` : ''}
                      >
                        {row[col] == null ? (
                          <span className="table-null">null</span>
                        ) : (
                          String(row[col])
                        )}
                      </td>
                    ))}
                  </tr>
                );
              })}
            </tbody>
          </table>
          <div className="table-widget-footer" style={{ borderTop: '1px solid var(--border-color)', padding: '6px 12px' }}>
            <span>
              Showing {sortedRows.length} of {widget.result!.rowCount} rows
            </span>
            {sortCol && (
              <button className="table-sort-clear" onClick={() => setSortCol(null)}>
                Clear sort
              </button>
            )}
          </div>
        </div>
      )}
    </WidgetShell>
  );
}
