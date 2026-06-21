import { useState } from 'react';
import { useBenchmarkStore } from '../../store/benchmarkStore';
import { useDashboardStore } from '../../store/dashboardStore';
import type { BenchmarkSummary, BenchmarkEvent } from '../../analytics/registry/types';

const handleDownloadJSON = (summary: BenchmarkSummary, events: BenchmarkEvent[]) => {
  const dataStr = JSON.stringify({ summary, events }, null, 2);
  const blob = new Blob([dataStr], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `transit-intelligence-session-diagnostics-${Date.now()}.json`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
};

export default function DiagnosticsPanel() {
  const { events, clearEvents, getSummary } = useBenchmarkStore();
  const widgets = useDashboardStore((s) => s.widgets);
  const [isOpen, setIsOpen] = useState(true);

  const summary = getSummary();

  const getWidgetTitle = (widgetId?: string) => {
    if (!widgetId) return null;
    const w = widgets.find((x) => x.id === widgetId);
    return w ? w.title : `Widget [${widgetId.slice(0, 8)}]`;
  };

  // Color mappings for different event types
  const getEventBadgeClass = (type: string) => {
    if (type.includes('render')) return 'bg-amber-500/10 text-amber-400 border-amber-500/20';
    if (type.includes('query')) return 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20';
    if (type === 'cross-highlight') return 'bg-pink-500/10 text-pink-400 border-pink-500/20';
    if (type === 'join-planning') return 'bg-indigo-500/10 text-indigo-400 border-indigo-500/20';
    if (type === 'schema-discovery') return 'bg-blue-500/10 text-blue-400 border-blue-500/20';
    return 'bg-slate-500/10 text-slate-400 border-slate-500/20';
  };

  return (
    <aside 
      className={`relative flex flex-col bg-slate-900 border-l border-slate-800 text-slate-300 select-none overflow-hidden h-full flex-shrink-0 transition-all duration-300 ${
        isOpen ? 'w-80' : 'w-12'
      }`}
    >
      {/* Toggle Button / Header tab when collapsed */}
      {!isOpen ? (
        <button 
          onClick={() => setIsOpen(true)}
          className="w-full h-full flex flex-col items-center justify-start py-8 gap-6 text-slate-400 hover:text-white cursor-pointer"
          title="Open Diagnostics Console"
        >
          <span className="text-xs">◀</span>
          <span className="text-[10px] uppercase font-bold tracking-widest [writing-mode:vertical-lr]">
            Observability
          </span>
          {events.length > 0 && (
            <span className="text-[9px] bg-indigo-500 text-white w-5 h-5 rounded-full flex items-center justify-center font-mono font-bold animate-pulse">
              {events.length}
            </span>
          )}
        </button>
      ) : (
        <>
          {/* Header */}
          <div className="p-4 border-b border-slate-800 flex items-center justify-between">
            <div className="flex items-center gap-2">
              <span className="text-xs font-semibold uppercase tracking-wider text-slate-400">
                Observability Panel
              </span>
              {events.length > 0 && (
                <span className="text-[9px] bg-slate-850 border border-slate-700 text-indigo-300 font-mono px-1.5 py-0.5 rounded">
                  {events.length} events
                </span>
              )}
            </div>
            <button 
              onClick={() => setIsOpen(false)}
              className="text-slate-500 hover:text-slate-300 text-xs px-1.5 py-1 rounded hover:bg-slate-800 cursor-pointer transition-colors"
              title="Collapse Panel"
            >
              ▶
            </button>
          </div>

          {/* Metrics summary scrollable */}
          <div className="flex-1 overflow-y-auto custom-scrollbar p-3 space-y-4">
            
            {/* KPI section */}
            <div>
              <span className="block text-[10px] font-bold text-slate-500 uppercase tracking-widest mb-2">
                Session Performance
              </span>
              <div className="grid grid-cols-2 gap-2">
                <div className="bg-slate-950/40 border border-slate-800/80 p-2.5 rounded">
                  <span className="block text-[9px] uppercase text-slate-500 font-medium">Avg Query</span>
                  <span className="text-sm font-bold font-mono text-slate-200">
                    {summary.avgQueryMs > 0 ? `${summary.avgQueryMs.toFixed(1)}ms` : '—'}
                  </span>
                </div>
                <div className="bg-slate-950/40 border border-slate-800/80 p-2.5 rounded">
                  <span className="block text-[9px] uppercase text-slate-500 font-medium">Avg Render</span>
                  <span className="text-sm font-bold font-mono text-slate-200">
                    {summary.avgRenderMs > 0 ? `${summary.avgRenderMs.toFixed(1)}ms` : '—'}
                  </span>
                </div>
                <div className="bg-slate-950/40 border border-slate-800/80 p-2.5 rounded">
                  <span className="block text-[9px] uppercase text-slate-500 font-medium">Avg Highlight</span>
                  <span className="text-sm font-bold font-mono text-slate-200">
                    {summary.avgHighlightMs > 0 ? `${summary.avgHighlightMs.toFixed(1)}ms` : '—'}
                  </span>
                </div>
                <div className="bg-slate-950/40 border border-slate-800/80 p-2.5 rounded">
                  <span className="block text-[9px] uppercase text-slate-500 font-medium">95th % Query</span>
                  <span className="text-sm font-bold font-mono text-slate-200">
                    {summary.pct95QueryMs > 0 ? `${summary.pct95QueryMs.toFixed(1)}ms` : '—'}
                  </span>
                </div>
              </div>
            </div>

            {/* General Counters */}
            <div className="bg-slate-950/20 border border-slate-800/70 p-3 rounded space-y-2">
              <span className="block text-[10px] font-bold text-slate-500 uppercase tracking-widest border-b border-slate-800 pb-1.5 mb-1.5">
                Diagnostics Details
              </span>
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-slate-400">Dashboard Load:</span>
                <span className="font-mono text-slate-200">{summary.dashboardLoadMs.toFixed(0)}ms</span>
              </div>
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-slate-400">Total Executed Queries:</span>
                <span className="font-mono text-slate-200">{summary.totalQueries}</span>
              </div>
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-slate-400">Total Rows Retrieved:</span>
                <span className="font-mono text-slate-200">{summary.totalRows.toLocaleString()}</span>
              </div>
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-slate-400">Slowest Query:</span>
                <span className="font-mono text-slate-200">
                  {summary.slowestQueryMs > 0 ? `${summary.slowestQueryMs.toFixed(1)}ms` : '—'}
                </span>
              </div>
              <div className="flex items-center justify-between text-[11px]">
                <span className="text-slate-400">Largest Result Set:</span>
                <span className="font-mono text-slate-200">
                  {summary.largestResult > 0 ? `${summary.largestResult.toLocaleString()} rows` : '—'}
                </span>
              </div>
              {summary.avgHighlightMs > 0 && summary.avgHighlightMs < 100 && (
                <div className="text-[10px] bg-emerald-950/20 text-emerald-400 border border-emerald-800/30 p-1.5 rounded text-center font-medium mt-2">
                  ✓ ADR 0012 — sub-100ms cross-filtering
                </div>
              )}
            </div>

            {/* Live Activities ticker */}
            <div className="flex-1 flex flex-col min-height-[250px]">
              <span className="block text-[10px] font-bold text-slate-500 uppercase tracking-widest mb-2 border-b border-slate-800 pb-1.5">
                Telemetry Log
              </span>
              <div className="flex-1 overflow-y-auto max-h-[300px] bg-slate-950/50 rounded border border-slate-850 p-2 space-y-1.5 custom-scrollbar font-mono text-[10px] leading-relaxed">
                {events.length === 0 ? (
                  <div className="text-slate-600 text-center py-8">
                    No telemetry events yet.
                  </div>
                ) : (
                  events.map((e) => {
                    const timeString = new Date(e.timestamp).toLocaleTimeString(undefined, {
                      hour12: false,
                      hour: '2-digit',
                      minute: '2-digit',
                      second: '2-digit',
                    }) + `.${(e.timestamp % 1000).toString().padStart(3, '0')}`;
                    
                    const title = getWidgetTitle(e.widgetId);

                    return (
                      <div key={e.id} className="border-b border-slate-850 pb-1.5 last:border-b-0 space-y-0.5">
                        <div className="flex items-center justify-between gap-1">
                          <span className="text-slate-600 text-[9px]">{timeString}</span>
                          <span className={`px-1 rounded text-[8px] border font-bold ${getEventBadgeClass(e.eventType)}`}>
                            {e.eventType}
                          </span>
                        </div>
                        <div className="flex justify-between gap-2 text-slate-400">
                          <span className="truncate text-indigo-300/80">
                            {title ? `"${title}"` : 'Studio System'}
                          </span>
                          <span className="text-slate-200 font-bold whitespace-nowrap">
                            {e.durationMs.toFixed(1)}ms
                          </span>
                        </div>
                        {e.rowsReturned != null && (
                          <div className="text-slate-600 text-[9px]">
                            Returned {e.rowsReturned.toLocaleString()} rows
                          </div>
                        )}
                        {e.metadata?.sql && (
                          <div className="mt-1">
                            <div className="text-[8px] uppercase tracking-wider text-slate-500 font-bold mb-0.5">SQL Query:</div>
                            <pre className="p-1.5 bg-slate-950/80 border border-slate-800/80 rounded text-[9px] text-indigo-300 font-mono overflow-x-auto whitespace-pre max-h-32 custom-scrollbar">
                              {String(e.metadata.sql)}
                            </pre>
                          </div>
                        )}
                      </div>
                    );
                  })
                )}
              </div>
            </div>

          </div>

          {/* Action buttons */}
          <div className="p-3 border-t border-slate-800 bg-slate-950/40 flex items-center gap-2">
            <button
              onClick={() => handleDownloadJSON(summary, events)}
              className="flex-1 py-1.5 px-3 bg-slate-800 hover:bg-slate-700 text-white rounded text-[11px] font-semibold text-center transition-colors cursor-pointer border border-slate-700"
            >
              📥 Download Log
            </button>
            <button
              onClick={clearEvents}
              className="py-1.5 px-3 bg-slate-950 hover:bg-red-950/20 text-slate-500 hover:text-red-400 rounded text-[11px] font-semibold text-center transition-colors border border-slate-850 hover:border-red-900/30 cursor-pointer"
            >
              Clear
            </button>
          </div>
        </>
      )}
    </aside>
  );
}
