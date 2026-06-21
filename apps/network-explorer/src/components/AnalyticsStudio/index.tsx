import { useEffect } from 'react';
import { useDashboardStore } from '../../store/dashboardStore';
import DataPane from './DataPane';
import ConfigPane from './ConfigPane';
import DashboardCanvas from './DashboardCanvas';
import DiagnosticsPanel from './DiagnosticsPanel';

//-------------------------------------
// Analytics Workbench — Page Shell
//
// Layout:
//   [DataPane 288px] | [ConfigPane 240px] | [Canvas area: flex-column] | [DiagnosticsPanel 320px]
//
// Catalog discovery runs once on mount. The DataPane, ConfigPane, Canvas, and Observability
// read catalog state from the store directly.
//-------------------------------------

export default function AnalyticsStudio() {
  const initCatalog = useDashboardStore((s) => s.initCatalog);
  const filterContext = useDashboardStore((s) => s.filterContext);
  const theme = useDashboardStore((s) => s.theme);

  // Discover the DuckDB catalog once when the studio mounts.
  useEffect(() => {
    void initCatalog();
  }, [initCatalog]);

  return (
    <div className={`studio-root theme-${theme}`}>
      {/* Studio header */}
      <header className="studio-header">
        <div className="studio-header-inner">
          <div className="studio-header-title-group">
            <span className="studio-header-eyebrow">Transit Intelligence</span>
            <h1 className="studio-header-title">Analytics Workbench</h1>
          </div>
          <div className="studio-header-stats flex items-center gap-3">
            {filterContext.value && (
              <span className="studio-stat-chip studio-stat-chip--active">
                ⚡ Filter active
              </span>
            )}
          </div>
        </div>
      </header>

      {/* Main workbench area */}
      <div className="workbench-layout">
        <DataPane />
        <ConfigPane />
        <div className="workbench-canvas-area">
          <DashboardCanvas />
        </div>
        <DiagnosticsPanel />
      </div>
    </div>
  );
}

