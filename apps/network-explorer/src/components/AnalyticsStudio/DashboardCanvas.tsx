import { useDashboardStore } from '../../store/dashboardStore';
import type { Widget } from '../../analytics/registry/types';
import ChartWidget from './widgets/ChartWidget';
import TableWidget from './widgets/TableWidget';
import MetricCardWidget from './widgets/MetricCardWidget';
import MapWidget from './widgets/MapWidget';
import MatrixWidget from './widgets/MatrixWidget';

//-------------------------------------
// Dashboard Canvas — Multi-Widget Grid
//
// Renders the widget grid. Maps the flattened taxonomy to their respective components.
// Layout: CSS grid that accommodates diverse layout sizes naturally.
//-------------------------------------

function renderWidget(widget: Widget) {
  switch (widget.type) {
    case 'bar':
    case 'bar-horizontal':
    case 'line':
    case 'pie':
    case 'scatter':
      return <ChartWidget key={widget.id} widget={widget} />;
    case 'table':
      return <TableWidget key={widget.id} widget={widget} />;
    case 'matrix':
      return <MatrixWidget key={widget.id} widget={widget} />;
    case 'map':
      return <MapWidget key={widget.id} widget={widget} />;
    case 'metric':
      return <MetricCardWidget key={widget.id} widget={widget} />;
  }
}

export default function DashboardCanvas() {
  const widgets = useDashboardStore((s) => s.widgets);
  const highlightContext = useDashboardStore((s) => s.highlightContext);
  const clearHighlight = useDashboardStore((s) => s.clearHighlight);

  return (
    <div className="dashboard-canvas">
      {/* Active highlight context banner */}
      {highlightContext.value && (
        <div className="canvas-filter-banner bg-pink-900/10 border-pink-900/20 text-pink-300" role="status" aria-live="polite">
          <span className="canvas-filter-icon text-pink-400">⚡</span>
          <span className="flex-1">
            Highlighting: <strong>{highlightContext.column?.split('.').pop()}</strong> ={' '}
            <strong>&ldquo;{highlightContext.value}&rdquo;</strong>
            {' '}— matching items highlighted; non-matching filtered or faded.
          </span>
          <button 
            onClick={clearHighlight} 
            className="px-2 py-0.5 rounded text-xs bg-pink-900/30 border border-pink-700/50 hover:bg-pink-900/50 text-pink-300 pointer-events-auto cursor-pointer"
          >
            ✕ Clear highlight
          </button>
        </div>
      )}

      {/* Widget grid */}
      {widgets.length === 0 ? (
        <div className="canvas-empty">
          <div className="canvas-empty-icon" aria-hidden="true">
            <svg width="56" height="56" viewBox="0 0 24 24" fill="none"
              stroke="currentColor" strokeWidth="1" strokeLinecap="round" strokeLinejoin="round">
              <rect x="3" y="3" width="7" height="7" rx="1" />
              <rect x="14" y="3" width="7" height="7" rx="1" />
              <rect x="14" y="14" width="7" height="7" rx="1" />
              <rect x="3" y="14" width="7" height="7" rx="1" />
            </svg>
          </div>
          <p className="canvas-empty-title">No widgets yet</p>
          <p className="canvas-empty-hint">
            Use the <strong>Build</strong> tab in the sidebar to add your first widget.
          </p>
        </div>
      ) : (
        <div className="canvas-grid">
          {widgets.map((w) => renderWidget(w))}
        </div>
      )}
    </div>
  );
}
