import type { Widget } from '../../../analytics/registry/types';
import WidgetShell from './WidgetShell';

//-------------------------------------
// Metric Card Widget (KPI Card)
//
// Displays a single aggregated value in a large, prominent format.
// Useful for total counts, averages, and scalar summaries.
//
// The metric query always returns a single row with a `value` column.
// (Enforced by workbenchGenerator.ts buildMetricSql.)
//-------------------------------------

interface Props {
  widget: Widget;
}

/** Format a numeric value into a human-readable string with k/M suffix. */
function formatValue(raw: unknown): string {
  const n = Number(raw);
  if (isNaN(n)) return String(raw ?? '—');
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(2)}M`;
  if (n >= 10_000) return `${(n / 1_000).toFixed(1)}k`;
  return n.toLocaleString();
}

export default function MetricCardWidget({ widget }: Props) {
  const row = widget.result?.rows[0];
  const rawValue = row?.value;
  const displayValue = rawValue != null ? formatValue(rawValue) : '—';
  const hasResult = widget.result != null && !widget.isLoading;

  return (
    <WidgetShell widget={widget}>
      <div className="metric-card-body">
        <div className="metric-card-value" title={String(rawValue ?? '')}>
          {displayValue}
        </div>
        <div className="metric-card-label">{widget.measureCol ?? 'value'}</div>
        {hasResult && (
          <div className="metric-card-meta">
            <span className="metric-card-exec">
              {widget.result!.executionMs.toFixed(1)} ms
            </span>
          </div>
        )}
      </div>
    </WidgetShell>
  );
}
