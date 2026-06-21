import { useState, useEffect, useRef } from 'react';
import type { ReactNode } from 'react';
import type { Widget } from '../../../analytics/registry/types';
import { useDashboardStore } from '../../../store/dashboardStore';

const THRESHOLDS: Record<string, { w: number; h: number; symbol: string; label: string }> = {
  bar: { w: 240, h: 200, symbol: '📊', label: 'Bar Chart' },
  'bar-horizontal': { w: 240, h: 200, symbol: '📊', label: 'Horizontal Bar Chart' },
  line: { w: 240, h: 200, symbol: '📈', label: 'Line Chart' },
  pie: { w: 240, h: 200, symbol: '🥧', label: 'Pie Chart' },
  scatter: { w: 240, h: 200, symbol: '🟰', label: 'Scatter Plot' },
  table: { w: 200, h: 150, symbol: '📋', label: 'Table' },
  matrix: { w: 220, h: 160, symbol: '🔲', label: 'Matrix' },
  map: { w: 240, h: 200, symbol: '🗺️', label: 'Map' },
  metric: { w: 150, h: 100, symbol: '🔢', label: 'Metric KPI' },
};

const WIDGET_LABELS: Record<string, string> = {
  bar: 'BAR',
  'bar-horizontal': 'HBAR',
  line: 'LINE',
  pie: 'PIE',
  scatter: 'SCAT',
  table: 'TBL',
  matrix: 'MTX',
  map: 'MAP',
  metric: 'KPI',
};

interface Props {
  widget: Widget;
  children: ReactNode;
}

export default function WidgetShell({ widget, children }: Props) {
  const removeWidget = useDashboardStore((s) => s.removeWidget);
  const duplicateWidget = useDashboardStore((s) => s.duplicateWidget);
  const selectedWidgetId = useDashboardStore((s) => s.selectedWidgetId);
  const setSelectedWidgetId = useDashboardStore((s) => s.setSelectedWidgetId);

  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 });

  // Observe dimensions locally
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const observer = new ResizeObserver((entries) => {
      if (entries[0]) {
        const { width, height } = entries[0].contentRect;
        setDimensions({ width, height });
      }
    });
    observer.observe(el);

    return () => observer.disconnect();
  }, []);

  const threshold = THRESHOLDS[widget.type] || { w: 150, h: 100, symbol: '⬜', label: 'Widget' };
  const isTooSmall = dimensions.width > 0 && dimensions.height > 0 &&
    (dimensions.width < threshold.w || dimensions.height < threshold.h);

  const isSelected = selectedWidgetId === widget.id;

  return (
    <div 
      ref={containerRef}
      className={`widget-shell widget-shell--${widget.type} relative flex flex-col bg-slate-900 border rounded-lg shadow-md overflow-hidden transition-all duration-200 hover:shadow-lg cursor-pointer ${
        isSelected ? 'widget-shell--selected border-indigo-500 ring-2 ring-indigo-500/20' : 'border-slate-800'
      }`}
      style={{
        resize: 'both',
        overflow: 'hidden',
        minWidth: '120px',
        minHeight: '80px',
      }}
      id={`widget-${widget.id}`}
      onClick={() => setSelectedWidgetId(widget.id)}
    >
      {/* Header */}
      <div className="widget-shell-header flex items-center justify-between px-3 py-2 bg-slate-950 border-b border-slate-850 select-none">
        <div className="flex items-center gap-2 overflow-hidden flex-1 mr-2">
          <span className="widget-type-badge text-[9px] font-bold bg-indigo-500/10 text-indigo-400 border border-indigo-500/20 px-1 rounded flex-shrink-0">
            {WIDGET_LABELS[widget.type] || 'WIDG'}
          </span>
          <span className="widget-title text-xs font-semibold text-slate-200 truncate" title={widget.title}>
            {widget.title}
          </span>
        </div>

        <div className="flex items-center gap-1.5 flex-shrink-0 text-[10px]" onClick={(e) => e.stopPropagation()}>
          {widget.result && !isTooSmall && (
            <span className="widget-exec-time text-slate-500 font-mono" title="Query execution time">
              {widget.result.executionMs.toFixed(0)}ms
            </span>
          )}
          {isTooSmall && (
            <span className="text-amber-500 font-mono text-[9px]" title="Resize to see data">
              Too Small
            </span>
          )}
          
          <div className="h-3 w-px bg-slate-800" />

          {/* Select toggle */}
          <button
            onClick={() => setSelectedWidgetId(isSelected ? null : widget.id)}
            className={`p-1 rounded text-slate-400 hover:text-white hover:bg-slate-800 cursor-pointer transition-colors ${
              isSelected ? 'bg-slate-800 text-indigo-400' : ''
            }`}
            title={isSelected ? "Clear selection" : "Select visualization"}
          >
            ⚙️
          </button>

          {/* Duplicate button */}
          <button
            onClick={() => duplicateWidget(widget.id)}
            className="p-1 rounded text-slate-400 hover:text-white hover:bg-slate-800 cursor-pointer transition-colors"
            title="Duplicate widget block"
          >
            📋
          </button>

          {/* Delete button */}
          <button
            onClick={() => removeWidget(widget.id)}
            className="p-1 rounded text-slate-400 hover:text-red-400 hover:bg-slate-800 cursor-pointer transition-colors"
            title="Remove widget"
          >
            ✕
          </button>
        </div>
      </div>

      {/* Body container */}
      <div className="widget-shell-body flex-1 relative min-h-0 bg-slate-900/40">
        
        {/* Loading Indicator */}
        {widget.isLoading && (
          <div className="absolute inset-0 z-10 flex items-center justify-center bg-slate-950/70 backdrop-blur-[1px]">
            <div className="studio-loading-ring">
              <div /><div /><div /><div />
            </div>
          </div>
        )}

        {/* Fallback layout if container is too small */}
        {isTooSmall ? (
          <div className="absolute inset-0 flex flex-col items-center justify-center bg-slate-950 p-2 text-center select-none">
            <span className="text-3xl mb-1 filter drop-shadow">{threshold.symbol}</span>
            <span className="text-[10px] text-slate-500 font-medium truncate max-w-full">
              {threshold.label}
            </span>
            <span className="text-[9px] text-slate-600 font-mono mt-1">
              {dimensions.width}×{dimensions.height}
            </span>
          </div>
        ) : widget.error ? (
          /* Error State */
          <div className="absolute inset-0 flex flex-col items-center justify-center p-4 text-center bg-slate-950/50">
            <span className="text-xl text-red-500 mb-1.5">⚠</span>
            <span className="text-[11px] text-red-400 font-medium max-w-full line-clamp-3 overflow-hidden">
              {widget.error}
            </span>
          </div>
        ) : (
          /* Normal Visualisation Content */
          children
        )}
      </div>

      {/* Sizing coordinate text inside absolute overlay when resizing */}
      <div className="absolute bottom-1 right-1 text-[8px] text-slate-600 font-mono pointer-events-none select-none">
        {dimensions.width}×{dimensions.height}
      </div>
    </div>
  );
}
