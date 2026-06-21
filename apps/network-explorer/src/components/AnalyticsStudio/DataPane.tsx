import { useState } from 'react';
import { useDashboardStore } from '../../store/dashboardStore';
import type { 
  WidgetType, 
  ColumnKind, 
  WidgetConfig 
} from '../../analytics/registry/types';
import { GTFS_RELATIONSHIPS } from '../../analytics/semantic/relationships';

const KIND_LABEL: Record<ColumnKind, string> = {
  dimension: 'D',
  measure: 'M',
  geo: 'G',
  id: 'ID',
};

const KIND_TITLE: Record<ColumnKind, string> = {
  dimension: 'Dimension',
  measure: 'Measure',
  geo: 'Geographical',
  id: 'Identifier',
};

const KIND_COLOR_CLASS: Record<ColumnKind, string> = {
  dimension: 'bg-blue-500/10 text-blue-400 border-blue-500/20',
  measure: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
  geo: 'bg-amber-500/10 text-amber-400 border-amber-500/20',
  id: 'bg-slate-500/10 text-slate-400 border-slate-500/20',
};

export default function DataPane() {
  const catalog = useDashboardStore((s) => s.catalog);
  const addWidget = useDashboardStore((s) => s.addWidget);
  
  const [isOpen, setIsOpen] = useState(true);
  const [searchTerm, setSearchTerm] = useState('');
  const [expandedTables, setExpandedTables] = useState<Set<string>>(new Set(['stops', 'routes']));
  const [activeColumnMenu, setActiveColumnMenu] = useState<{ table: string; column: string } | null>(null);

  const toggleTable = (name: string) => {
    setExpandedTables((prev) => {
      const next = new Set(prev);
      if (next.has(name)) {
        next.delete(name);
      } else {
        next.add(name);
      }
      return next;
    });
  };

  const handleQuickAdd = (tableName: string, colName: string, kind: ColumnKind, type: WidgetType) => {
    const fieldName = `${tableName}.${colName}`;
    let config: WidgetConfig;

    if (type === 'metric') {
      config = {
        type: 'metric',
        title: `Total ${colName}`,
        tableName,
        dimensionCol: null,
        measureCol: fieldName,
        aggregation: kind === 'measure' ? 'SUM' : 'COUNT',
      };
    } else if (type === 'map') {
      config = {
        type: 'map',
        title: `Geographic distribution of ${colName}`,
        tableName,
        dimensionCol: fieldName,
        measureCol: null,
        aggregation: 'none',
      };
    } else if (type === 'table') {
      config = {
        type: 'table',
        title: `${tableName} Detail View`,
        tableName,
        dimensionCol: null,
        measureCol: null,
        aggregation: 'none',
        tableColumns: [
          { name: fieldName, aggregation: 'none' }
        ],
      };
    } else {
      // chart types: bar, line, pie, etc.
      // Find a suitable counter-part (e.g. if we clicked a dimension, we aggregate a measure or COUNT)
      const firstMeasure = catalog
        .find((t) => t.name === tableName)
        ?.columns.find((c) => c.kind === 'measure');

      config = {
        type,
        title: `${colName} Distribution`,
        tableName,
        dimensionCol: kind === 'dimension' || kind === 'id' ? fieldName : null,
        measureCol: kind === 'measure' ? fieldName : (firstMeasure ? `${tableName}.${firstMeasure.name}` : null),
        aggregation: kind === 'measure' ? 'SUM' : 'COUNT',
      };
    }

    addWidget(config);
    setActiveColumnMenu(null);
  };

  // Filter catalog based on search
  const filteredCatalog = catalog.map((table) => {
    const matchesTable = table.name.toLowerCase().includes(searchTerm.toLowerCase());
    const matchedColumns = table.columns.filter((col) =>
      col.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
      col.dataType.toLowerCase().includes(searchTerm.toLowerCase())
    );

    if (matchesTable || matchedColumns.length > 0) {
      return {
        ...table,
        columns: matchesTable ? table.columns : matchedColumns,
      };
    }
    return null;
  }).filter(Boolean) as typeof catalog;

  if (!isOpen) {
    return (
      <aside className="w-12 flex flex-col bg-slate-900 border-r border-slate-800 text-slate-400 select-none overflow-hidden h-full flex-shrink-0 items-center py-4 gap-6 transition-all duration-300">
        <button 
          onClick={() => setIsOpen(true)}
          className="p-1.5 rounded hover:bg-slate-800 hover:text-white cursor-pointer transition-colors text-xs font-semibold"
          title="Open Data Explorer"
        >
          ▶
        </button>
        <span className="text-[10px] uppercase font-bold tracking-widest [writing-mode:vertical-lr] text-slate-500 whitespace-nowrap">
          Data Explorer
        </span>
      </aside>
    );
  }

  return (
    <aside className="w-72 flex flex-col bg-slate-900 border-r border-slate-800 text-slate-300 select-none overflow-hidden h-full flex-shrink-0">
      {/* Title */}
      <div className="p-4 border-b border-slate-800 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <svg className="w-4 height-4 text-indigo-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M4 7v10c0 2 1 3 3 3h10c2 0 3-1 3-3V7c0-2-1-3-3-3H7C5 4 4 5 4 7z" />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M9 17h6M9 13h6M9 9h6" />
          </svg>
          <span className="font-semibold text-xs tracking-wider uppercase text-slate-400">Data Explorer</span>
        </div>
        <div className="flex items-center gap-2">
          <span className="text-[10px] bg-slate-800 text-slate-400 px-2 py-0.5 rounded font-mono">
            GTFS
          </span>
          <button 
            onClick={() => setIsOpen(false)}
            className="text-slate-500 hover:text-slate-350 text-xs px-1.5 py-0.5 rounded hover:bg-slate-800 cursor-pointer transition-colors"
            title="Collapse Data Explorer"
          >
            ◀
          </button>
        </div>
      </div>

      {/* Search Bar */}
      <div className="p-3 border-b border-slate-800">
        <div className="relative">
          <input
            type="text"
            className="w-full bg-slate-950/80 border border-slate-800 hover:border-slate-700/80 focus:border-indigo-500 rounded px-2.5 py-1.5 pl-8 text-xs font-medium placeholder-slate-500 text-slate-200 outline-none transition-colors"
            placeholder="Search columns or tables..."
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
          />
          <svg className="absolute left-2.5 top-2.5 w-3.5 h-3.5 text-slate-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
        </div>
      </div>

      {/* Tables Tree */}
      <div className="flex-1 overflow-y-auto custom-scrollbar p-2 space-y-1">
        {filteredCatalog.length === 0 ? (
          <div className="text-center py-8 text-slate-500 text-xs">
            No columns match your search
          </div>
        ) : (
          filteredCatalog.map((table) => {
            const isExpanded = expandedTables.has(table.name);
            return (
              <div key={table.name} className="rounded border border-slate-800/20 overflow-hidden">
                <button
                  className={`w-full flex items-center gap-2 px-2.5 py-2 text-left hover:bg-slate-800/40 text-xs font-mono font-bold transition-colors ${
                    isExpanded ? 'bg-slate-800/20 text-indigo-300' : 'text-slate-300'
                  }`}
                  onClick={() => toggleTable(table.name)}
                >
                  <span className="text-[10px] text-slate-500">
                    {isExpanded ? '▼' : '▶'}
                  </span>
                  <span className="flex-1 truncate">{table.name}</span>
                  <span className="text-[9px] bg-slate-950 px-1.5 py-0.5 rounded text-slate-500 font-normal">
                    {table.columns.length}
                  </span>
                </button>

                {isExpanded && (
                  <div className="bg-slate-950/20 border-t border-slate-800/30 py-1 pl-4 pr-1 space-y-0.5">
                    {table.columns.map((col) => {
                      const isMenuOpen = activeColumnMenu?.table === table.name && activeColumnMenu?.column === col.name;
                      return (
                        <div key={col.name} className="relative group">
                          <div 
                            className="flex items-center gap-2 py-1 px-1.5 rounded hover:bg-slate-800/60 cursor-pointer text-xs transition-colors active:bg-slate-800"
                            onClick={() => setActiveColumnMenu(isMenuOpen ? null : { table: table.name, column: col.name })}
                            draggable={true}
                            onDragStart={(e) => {
                              e.dataTransfer.setData('text/plain', `${table.name}.${col.name}`);
                              e.dataTransfer.effectAllowed = 'copy';
                            }}
                          >
                            <span 
                              className={`w-5 h-4 flex items-center justify-center text-[9px] font-bold border rounded flex-shrink-0 ${KIND_COLOR_CLASS[col.kind]}`}
                              title={KIND_TITLE[col.kind]}
                            >
                              {KIND_LABEL[col.kind]}
                            </span>
                            <span className="flex-1 font-mono truncate text-slate-300 text-[11px]">
                              {col.name}
                            </span>
                            <span className="text-[9px] text-slate-600 font-mono pr-1">
                              {col.dataType}
                            </span>
                          </div>

                          {/* Quick Add Menu */}
                          {isMenuOpen && (
                            <div className="absolute left-6 top-6 z-30 w-44 bg-slate-800 border border-slate-700 rounded-lg shadow-xl py-1 text-[11px]">
                              <div className="px-2.5 py-1 border-b border-slate-700 text-[9px] text-slate-500 uppercase tracking-wider font-bold">
                                Create Widget
                              </div>
                              <button 
                                onClick={() => handleQuickAdd(table.name, col.name, col.kind, 'bar')}
                                className="w-full text-left px-3 py-1.5 hover:bg-slate-750 text-slate-300 hover:text-white transition-colors"
                              >
                                📊 Bar Chart
                              </button>
                              <button 
                                onClick={() => handleQuickAdd(table.name, col.name, col.kind, 'line')}
                                className="w-full text-left px-3 py-1.5 hover:bg-slate-750 text-slate-300 hover:text-white transition-colors"
                              >
                                📈 Line Chart
                              </button>
                              <button 
                                onClick={() => handleQuickAdd(table.name, col.name, col.kind, 'pie')}
                                className="w-full text-left px-3 py-1.5 hover:bg-slate-750 text-slate-300 hover:text-white transition-colors"
                              >
                                🥧 Pie Chart
                              </button>
                              <button 
                                onClick={() => handleQuickAdd(table.name, col.name, col.kind, 'metric')}
                                className="w-full text-left px-3 py-1.5 hover:bg-slate-750 text-slate-300 hover:text-white transition-colors"
                              >
                                🔢 KPI Card
                              </button>
                              <button 
                                onClick={() => handleQuickAdd(table.name, col.name, col.kind, 'table')}
                                className="w-full text-left px-3 py-1.5 hover:bg-slate-750 text-slate-300 hover:text-white transition-colors"
                              >
                                📋 Detail Table
                              </button>
                              {col.kind === 'geo' && (
                                <button 
                                  onClick={() => handleQuickAdd(table.name, col.name, col.kind, 'map')}
                                  className="w-full text-left px-3 py-1.5 hover:bg-slate-750 text-slate-300 hover:text-white transition-colors"
                                >
                                  🗺️ Map View
                                </button>
                              )}
                              <div className="border-t border-slate-700 my-0.5" />
                              <button 
                                onClick={() => setActiveColumnMenu(null)}
                                className="w-full text-left px-3 py-1 hover:bg-slate-750 text-slate-500 hover:text-slate-350 transition-colors"
                              >
                                Cancel
                              </button>
                            </div>
                          )}
                        </div>
                      );
                    })}
                  </div>
                )}
              </div>
            );
          })
        )}
      </div>

      {/* Relational Links */}
      <div className="border-t border-slate-800 bg-slate-950/40 p-4">
        <span className="block text-[10px] font-semibold text-slate-500 uppercase tracking-wider mb-2.5">
          Discovered Joins
        </span>
        <div className="space-y-2">
          {GTFS_RELATIONSHIPS.map((rel, idx) => (
            <div 
              key={idx} 
              className="text-[10px] bg-slate-900/80 hover:bg-slate-900 border border-slate-800/85 p-2 rounded flex flex-col gap-1 transition-colors"
            >
              <div className="flex items-center justify-between text-slate-400">
                <span className="font-mono text-indigo-300 font-semibold">{rel.fromTable}</span>
                <span className="text-slate-600 font-bold">↔</span>
                <span className="font-mono text-indigo-300 font-semibold">{rel.toTable}</span>
              </div>
              <div className="font-mono text-[9px] text-slate-500 text-center bg-slate-950/60 py-0.5 rounded border border-slate-900/30">
                {rel.fromColumn} = {rel.toColumn}
              </div>
            </div>
          ))}
        </div>
      </div>
    </aside>
  );
}
