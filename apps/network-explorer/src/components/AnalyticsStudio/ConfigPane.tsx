import { useState } from 'react';
import { useDashboardStore } from '../../store/dashboardStore';
import type { 
  Widget, 
  WidgetType, 
  AggregationFn, 
  TableColumnConfig 
} from '../../analytics/registry/types';
import { getAggregationCapabilities, AGG_LABELS } from '../../analytics/semantic/measures';

const VISUAL_TYPES: { type: WidgetType; label: string; icon: string }[] = [
  { type: 'bar', label: 'Bar Chart', icon: '📊' },
  { type: 'bar-horizontal', label: 'H-Bar Chart', icon: '📊' },
  { type: 'line', label: 'Line Chart', icon: '📈' },
  { type: 'pie', label: 'Pie Chart', icon: '🥧' },
  { type: 'scatter', label: 'Scatter Plot', icon: '🟰' },
  { type: 'table', label: 'Detail Table', icon: '📋' },
  { type: 'matrix', label: 'Pivot Matrix', icon: '🔲' },
  { type: 'map', label: 'Map View', icon: '🗺️' },
  { type: 'metric', label: 'KPI Card', icon: '🔢' },
];

export default function ConfigPane() {
  const { widgets, selectedWidgetId, updateWidget, catalog, addWidget } = useDashboardStore();
  const activeWidget = widgets.find((w) => w.id === selectedWidgetId);
  const [dragOverZone, setDragOverZone] = useState<string | null>(null);
  const [isOpen, setIsOpen] = useState(true);

  // Table columns add-column state
  const [newColName, setNewColName] = useState('');
  const [newColAgg, setNewColAgg] = useState<AggregationFn>('none');
  const [newColAlias, setNewColAlias] = useState('');

  const handleAddNewWidget = () => {
    if (catalog.length === 0) return;
    const baseTable = catalog.find(t => t.name === 'stops') || catalog[0];
    const dim = baseTable.columns.find(c => c.kind === 'dimension') || baseTable.columns[0];
    const msr = baseTable.columns.find(c => c.kind === 'measure');
    
    addWidget({
      type: 'bar',
      title: `Distribution of ${dim.name}`,
      tableName: baseTable.name,
      dimensionCol: `${baseTable.name}.${dim.name}`,
      measureCol: msr ? `${baseTable.name}.${msr.name}` : null,
      aggregation: msr ? 'SUM' : 'COUNT',
      barStackMode: 'clustered',
    });
  };

  if (!isOpen) {
    return (
      <aside className="w-12 flex flex-col bg-slate-900 border-r border-slate-800 text-slate-400 select-none overflow-hidden h-full flex-shrink-0 items-center py-4 gap-6 transition-all duration-300">
        <button 
          onClick={() => setIsOpen(true)}
          className="p-1.5 rounded hover:bg-slate-800 hover:text-white cursor-pointer transition-colors text-xs font-semibold"
          title="Open Visual Config"
        >
          ▶
        </button>
        <span className="text-[10px] uppercase font-bold tracking-widest [writing-mode:vertical-lr] text-slate-500 whitespace-nowrap">
          Visual Config
        </span>
      </aside>
    );
  }

  if (!activeWidget) {
    return (
      <aside className="w-60 flex flex-col bg-slate-900 border-r border-slate-800 text-slate-400 select-none overflow-hidden h-full flex-shrink-0">
        <div className="p-4 border-b border-slate-800 flex items-center justify-between">
          <div className="flex items-center gap-2">
            <svg className="w-4 h-4 text-slate-500" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
            </svg>
            <span className="font-semibold text-xs tracking-wider uppercase text-slate-500">Visualizations</span>
          </div>
          <button 
            onClick={() => setIsOpen(false)}
            className="text-slate-500 hover:text-slate-350 text-xs px-1.5 py-0.5 rounded hover:bg-slate-800 cursor-pointer transition-colors"
            title="Collapse Visualizations"
          >
            ◀
          </button>
        </div>
        <div className="flex-1 flex flex-col justify-center items-center text-center p-4">
          <svg className="w-10 h-10 text-slate-700 mb-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="1.5" d="M15 15l-2 5L9 9l11 4-5 2zm0 0l5 5M7.188 2.239l.777 2.897M5.136 7.965l-2.898-.777M13.95 4.05l-2.122 2.122m-5.657 5.656l-2.12 2.122" />
          </svg>
          <p className="text-xs font-semibold text-slate-500 uppercase tracking-wider mb-1">Visual Config</p>
          <p className="text-[11px] text-slate-600 leading-normal mb-4">
            Select a widget on the canvas to customize visual fields and chart types.
          </p>
          <button
            onClick={handleAddNewWidget}
            disabled={catalog.length === 0}
            className="px-4 py-1.5 bg-indigo-650 hover:bg-indigo-600 active:bg-indigo-700 text-white rounded-md text-xs font-semibold border border-indigo-700 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
          >
            + Add Widget
          </button>
        </div>
      </aside>
    );
  }

  // Flattened columns list across all catalog tables
  const allColumns = catalog.flatMap((t) =>
    t.columns.map((c) => ({
      table: t.name,
      name: c.name,
      fullName: `${t.name}.${c.name}`,
      kind: c.kind,
      dataType: c.dataType,
    }))
  );

  const getAggsForColumn = (colName: string | null) => {
    if (!colName) return ['none', 'COUNT', 'COUNT_DISTINCT'] as AggregationFn[];
    const found = allColumns.find((c) => c.fullName === colName || c.name === colName);
    return found ? getAggregationCapabilities(found.kind) : ['none', 'COUNT', 'COUNT_DISTINCT'] as AggregationFn[];
  };

  const handleFieldChange = <K extends keyof Widget>(key: K, val: Widget[K]) => {
    updateWidget(activeWidget.id, { [key]: val });
  };

  const handleTypeChange = (newType: WidgetType) => {
    // Automatically preserve fields if compatible, reset others if needed
    updateWidget(activeWidget.id, { type: newType });
  };

  // Drag and Drop drops handling
  const handleDropField = (zone: string, columnName: string) => {
    if (zone === 'dimensionCol') {
      updateWidget(activeWidget.id, { dimensionCol: columnName });
    } else if (zone === 'measureCol') {
      const found = allColumns.find((c) => c.fullName === columnName);
      updateWidget(activeWidget.id, { 
        measureCol: columnName,
        aggregation: found?.kind === 'measure' ? 'SUM' : 'COUNT',
      });
    } else if (zone === 'matrixRowCol') {
      updateWidget(activeWidget.id, { matrixRowCol: columnName });
    } else if (zone === 'matrixColCol') {
      updateWidget(activeWidget.id, { matrixColCol: columnName });
    } else if (zone === 'legendCol') {
      updateWidget(activeWidget.id, { legendCol: columnName });
    } else if (zone === 'tableColumns') {
      const currentCols = activeWidget.tableColumns || [];
      const colConfig: TableColumnConfig = {
        name: columnName,
        aggregation: 'none',
        alias: columnName.split('.').pop(),
      };
      updateWidget(activeWidget.id, {
        tableColumns: [...currentCols, colConfig]
      });
    }
  };

  // Table column reorder and removal actions
  const handleRemoveTableCol = (idx: number) => {
    const current = activeWidget.tableColumns || [];
    updateWidget(activeWidget.id, {
      tableColumns: current.filter((_, i) => i !== idx)
    });
  };

  const handleMoveTableCol = (idx: number, dir: number) => {
    const current = [...(activeWidget.tableColumns || [])];
    const target = idx + dir;
    if (target >= 0 && target < current.length) {
      const tmp = current[idx];
      current[idx] = current[target];
      current[target] = tmp;
      updateWidget(activeWidget.id, { tableColumns: current });
    }
  };

  const handleAddCustomTableCol = () => {
    if (!newColName) return;
    const colConfig: TableColumnConfig = {
      name: newColName,
      aggregation: newColAgg,
      alias: newColAlias.trim() || newColName.split('.').pop(),
    };
    const current = activeWidget.tableColumns || [];
    updateWidget(activeWidget.id, {
      tableColumns: [...current, colConfig]
    });
    setNewColName('');
    setNewColAgg('none');
    setNewColAlias('');
  };

  // Render a single drag-drop zone (shelf)
  const renderShelf = (
    zoneId: string, 
    label: string, 
    currentVal: string | null, 
    placeholder: string
  ) => {
    const isOver = dragOverZone === zoneId;
    const colBaseName = currentVal ? currentVal.split('.').pop() : '';
    const colTableName = currentVal ? currentVal.split('.')[0] : '';

    return (
      <div 
        className="space-y-1"
        onDragOver={(e) => {
          e.preventDefault();
          setDragOverZone(zoneId);
        }}
        onDragLeave={() => setDragOverZone(null)}
        onDrop={(e) => {
          e.preventDefault();
          setDragOverZone(null);
          const colName = e.dataTransfer.getData('text/plain');
          if (colName) {
            handleDropField(zoneId, colName);
          }
        }}
      >
        <label className="text-[10px] uppercase font-bold tracking-wider text-slate-500 block">
          {label}
        </label>
        <div 
          className={`flex flex-col gap-1.5 p-2 rounded border transition-all ${
            isOver 
              ? 'border-indigo-500 bg-indigo-500/5 shadow-md shadow-indigo-500/5' 
              : currentVal 
                ? 'border-slate-800 bg-slate-950/20' 
                : 'border-dashed border-slate-800 bg-slate-950/40 text-slate-600 hover:border-slate-700/60'
          }`}
        >
          {currentVal ? (
            <div className="flex items-center justify-between gap-1 text-[11px]">
              <div className="flex items-center gap-1.5 overflow-hidden">
                <span className="text-[9px] bg-slate-800 text-slate-400 border border-slate-750 px-1 rounded font-mono truncate max-w-[65px]">
                  {colTableName}
                </span>
                <span className="font-mono text-slate-200 font-bold truncate">
                  {colBaseName}
                </span>
              </div>
              <button 
                onClick={() => updateWidget(activeWidget.id, { [zoneId as keyof Widget]: null })}
                className="text-slate-500 hover:text-slate-300 text-xs px-1"
                title="Clear field"
              >
                ✕
              </button>
            </div>
          ) : (
            <span className="text-[10px] text-center py-1 font-medium select-none">
              {placeholder}
            </span>
          )}

          {/* Backup dropdown selector */}
          <select
            className="w-full bg-slate-950/80 border border-slate-800 hover:border-slate-700/60 rounded px-1.5 py-1 text-[10px] outline-none font-mono text-slate-400 mt-1 cursor-pointer transition-colors"
            value={currentVal || ''}
            onChange={(e) => handleDropField(zoneId, e.target.value)}
          >
            <option value="">— select column —</option>
            {allColumns.map((c) => (
              <option key={c.fullName} value={c.fullName}>{c.fullName}</option>
            ))}
          </select>
        </div>
      </div>
    );
  };

  return (
    <aside className="w-60 flex flex-col bg-slate-900 border-r border-slate-800 text-slate-300 select-none overflow-hidden h-full flex-shrink-0">
      {/* Title */}
      <div className="p-4 border-b border-slate-800 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <svg className="w-4 h-4 text-indigo-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth="2" d="M12 6V4m0 2a2 2 0 100 4m0-4a2 2 0 110 4m-6 8a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4m6 6v10m6-2a2 2 0 100-4m0 4a2 2 0 110-4m0 4v2m0-6V4" />
          </svg>
          <span className="font-semibold text-xs tracking-wider uppercase text-slate-400">Visualizations</span>
        </div>
        <button 
          onClick={() => setIsOpen(false)}
          className="text-slate-500 hover:text-slate-350 text-xs px-1.5 py-0.5 rounded hover:bg-slate-800 cursor-pointer transition-colors"
          title="Collapse Visualizations"
        >
          ◀
        </button>
      </div>

      <div className="flex-1 overflow-y-auto custom-scrollbar p-3 space-y-4">
        
        {/* Add Widget Button */}
        <button
          onClick={handleAddNewWidget}
          disabled={catalog.length === 0}
          className="w-full py-2 bg-indigo-650 hover:bg-indigo-600 active:bg-indigo-700 text-white rounded-md text-xs font-semibold border border-indigo-700 cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed transition-colors text-center shadow shadow-indigo-950/20"
        >
          + Add Widget
        </button>

        {/* Viz Types Selectors */}
        <div>
          <span className="block text-[10px] font-bold text-slate-500 uppercase tracking-wider mb-2">
            Chart Type
          </span>
          <div className="grid grid-cols-3 gap-1 bg-slate-950/40 p-1 rounded-lg border border-slate-800">
            {VISUAL_TYPES.map((vt) => {
              const active = activeWidget.type === vt.type;
              return (
                <button
                  key={vt.type}
                  onClick={() => handleTypeChange(vt.type)}
                  className={`flex flex-col items-center justify-center p-1 rounded transition-all cursor-pointer text-center ${
                    active 
                      ? 'bg-indigo-650 text-white font-bold shadow' 
                      : 'hover:bg-slate-800/60 text-slate-400 hover:text-slate-200'
                  }`}
                  title={vt.label}
                >
                  <span className="text-base line-height-1">{vt.icon}</span>
                  <span className="text-[8px] font-medium uppercase tracking-tighter truncate max-w-full scale-[0.9] mt-0.5">
                    {vt.type.replace('-horizontal', ' h')}
                  </span>
                </button>
              );
            })}
          </div>
        </div>

        {/* Shelves Form context dependent */}
        <div className="space-y-3 pt-2 border-t border-slate-800">
          
          {/* Scatter settings */}
          {activeWidget.type === 'scatter' && (
            <>
              {renderShelf('dimensionCol', 'X Axis Column', activeWidget.dimensionCol, 'Drag X Column here')}
              {renderShelf('measureCol', 'Y Axis Column', activeWidget.measureCol, 'Drag Y Column here')}
            </>
          )}

          {/* Matrix settings */}
          {activeWidget.type === 'matrix' && (
            <>
              {renderShelf('matrixRowCol', 'Row Dimension', activeWidget.matrixRowCol || null, 'Drag Row Column here')}
              {renderShelf('matrixColCol', 'Column Dimension', activeWidget.matrixColCol || null, 'Drag Col Column here')}
              {renderShelf('measureCol', 'Value Measure', activeWidget.measureCol, 'Drag Value Column here')}
            </>
          )}

           {/* Map settings */}
          {activeWidget.type === 'map' && (
            <>
              {renderShelf('dimensionCol', 'Label Column', activeWidget.dimensionCol, 'Drag Label Column here')}
              {renderShelf('measureCol', 'Value Measure (Sizing)', activeWidget.measureCol, 'Drag Value Column here')}
              <div className="space-y-1.5 pt-2 border-t border-slate-800/65">
                <div className="flex items-center justify-between">
                  <label className="text-[10px] uppercase font-bold tracking-wider text-slate-500 block">
                    Dot Base Size
                  </label>
                  <span className="text-[10px] font-mono text-indigo-400 font-bold">
                    {activeWidget.mapPinSize || 6}px
                  </span>
                </div>
                <input
                  type="range"
                  min="2"
                  max="20"
                  step="1"
                  className="w-full h-1 bg-slate-950 rounded-lg appearance-none cursor-pointer accent-indigo-500"
                  value={activeWidget.mapPinSize || 6}
                  onChange={(e) => handleFieldChange('mapPinSize', Number(e.target.value))}
                />
                <div className="flex justify-between text-[8px] text-slate-600 font-mono">
                  <span>2px</span>
                  <span>11px</span>
                  <span>20px</span>
                </div>
              </div>
            </>
          )}

          {/* Metric settings */}
          {activeWidget.type === 'metric' && (
            <>
              {renderShelf('measureCol', 'Measure Column', activeWidget.measureCol, 'COUNT(*) — Drag measure')}
            </>
          )}

          {/* Standard Chart settings */}
          {activeWidget.type !== 'scatter' && 
            activeWidget.type !== 'matrix' && 
            activeWidget.type !== 'map' && 
            activeWidget.type !== 'metric' && 
            activeWidget.type !== 'table' && (
             <>
               {renderShelf('dimensionCol', 'Dimension (X-Axis)', activeWidget.dimensionCol, 'Drag Dimension here')}
               {renderShelf('measureCol', 'Measure (Y-Axis)', activeWidget.measureCol, 'Drag Measure here')}
               {renderShelf('legendCol', 'Legend / Grouping (Color)', activeWidget.legendCol || null, 'Drag Legend Column here')}

               {(activeWidget.type === 'bar' || activeWidget.type === 'bar-horizontal') && activeWidget.legendCol && (
                 <div className="space-y-1.5 pt-1">
                   <label className="text-[10px] uppercase font-bold tracking-wider text-slate-500 block">
                     Stack Mode
                   </label>
                   <div className="grid grid-cols-3 gap-1 bg-slate-950/40 p-1 rounded border border-slate-800">
                     {(['clustered', 'stacked', 'stacked-100'] as const).map((mode) => {
                       const active = (activeWidget.barStackMode || 'clustered') === mode;
                       const label = mode === 'clustered' ? 'Clustered' : mode === 'stacked' ? 'Stacked' : 'Stacked 100%';
                       return (
                         <button
                           key={mode}
                           onClick={() => handleFieldChange('barStackMode', mode)}
                           className={`py-1 rounded text-[9px] font-semibold cursor-pointer text-center transition-all ${
                             active 
                               ? 'bg-indigo-650 text-white font-bold shadow' 
                               : 'hover:bg-slate-800 text-slate-400 hover:text-slate-200'
                           }`}
                         >
                           {label}
                         </button>
                       );
                     })}
                   </div>
                 </div>
               )}
             </>
           )}

          {/* Table Settings column shelf Drop target */}
          {activeWidget.type === 'table' && (
            <div className="space-y-2.5">
              <span className="block text-[10px] font-bold text-slate-500 uppercase tracking-wider">
                Table Columns
              </span>
              
              {/* Drop Target Shelf for adding to Table columns */}
              <div 
                className={`p-2 rounded border-dashed border text-center transition-all ${
                  dragOverZone === 'tableColumns' 
                    ? 'border-indigo-500 bg-indigo-500/5 text-indigo-400 font-bold' 
                    : 'border-slate-800 bg-slate-950/40 text-slate-600 hover:border-slate-700/60'
                }`}
                onDragOver={(e) => {
                  e.preventDefault();
                  setDragOverZone('tableColumns');
                }}
                onDragLeave={() => setDragOverZone(null)}
                onDrop={(e) => {
                  e.preventDefault();
                  setDragOverZone(null);
                  const colName = e.dataTransfer.getData('text/plain');
                  if (colName) {
                    handleDropField('tableColumns', colName);
                  }
                }}
              >
                <span className="text-[10px] font-medium block">Drag column here to append</span>
              </div>

              {/* List Active table columns */}
              {(activeWidget.tableColumns || []).length > 0 && (
                <div className="space-y-1.5 max-h-48 overflow-y-auto bg-slate-950/60 p-2 border border-slate-850 rounded">
                  {activeWidget.tableColumns!.map((tc, idx) => (
                    <div 
                      key={idx} 
                      className="flex items-center gap-1 text-[10px] text-slate-300 font-mono py-1 px-1.5 bg-slate-900/60 rounded border border-slate-800/40"
                    >
                      <span className="flex-1 truncate" title={tc.name}>{tc.name.split('.').pop()}</span>
                      <span className="text-[8px] text-slate-500 bg-slate-950 px-1 rounded">{tc.aggregation}</span>
                      <button 
                        onClick={() => handleMoveTableCol(idx, -1)} 
                        disabled={idx === 0} 
                        className="px-0.5 text-[8px] disabled:opacity-30 cursor-pointer text-slate-500 hover:text-white"
                      >
                        ▲
                      </button>
                      <button 
                        onClick={() => handleMoveTableCol(idx, 1)} 
                        disabled={idx === activeWidget.tableColumns!.length - 1} 
                        className="px-0.5 text-[8px] disabled:opacity-30 cursor-pointer text-slate-500 hover:text-white"
                      >
                        ▼
                      </button>
                      <button 
                        onClick={() => handleRemoveTableCol(idx)} 
                        className="text-red-400 hover:text-red-300 px-1 cursor-pointer"
                      >
                        ✕
                      </button>
                    </div>
                  ))}
                </div>
              )}

              {/* Add column details fallback */}
              <div className="bg-slate-950/40 p-2 border border-slate-800 rounded space-y-2">
                <span className="block text-[9px] uppercase text-slate-500 font-bold">Select Column Dropdown</span>
                <select
                  className="w-full bg-slate-950 border border-slate-850 rounded px-1.5 py-1 text-[10px] font-mono outline-none"
                  value={newColName}
                  onChange={(e) => {
                    setNewColName(e.target.value);
                    setNewColAgg('none');
                  }}
                >
                  <option value="">— select column —</option>
                  {allColumns.map((c) => (
                    <option key={c.fullName} value={c.fullName}>{c.fullName}</option>
                  ))}
                </select>

                {newColName && (
                  <div className="grid grid-cols-2 gap-1.5">
                    <div>
                      <label className="text-[8px] uppercase text-slate-500 block mb-0.5">Agg</label>
                      <select
                        className="w-full bg-slate-950 border border-slate-850 rounded px-1.5 py-0.5 text-[9px] outline-none"
                        value={newColAgg}
                        onChange={(e) => setNewColAgg(e.target.value as AggregationFn)}
                      >
                        {getAggsForColumn(newColName).map((agg) => (
                          <option key={agg} value={agg}>{AGG_LABELS[agg]}</option>
                        ))}
                      </select>
                    </div>
                    <div>
                      <label className="text-[8px] uppercase text-slate-500 block mb-0.5">Alias</label>
                      <input
                        type="text"
                        placeholder="Optional"
                        className="w-full bg-slate-950 border border-slate-850 rounded px-1.5 py-0.5 text-[9px] outline-none text-slate-200"
                        value={newColAlias}
                        onChange={(e) => setNewColAlias(e.target.value)}
                      />
                    </div>
                  </div>
                )}

                <button
                  onClick={handleAddCustomTableCol}
                  disabled={!newColName}
                  className="w-full text-center py-1 bg-slate-800 hover:bg-slate-750 text-white rounded text-[10px] border border-slate-700 disabled:opacity-40 disabled:cursor-not-allowed cursor-pointer transition-colors"
                >
                  + Add Column
                </button>
              </div>
            </div>
          )}

          {/* Aggregation Function details (Only if measure is loaded and active) */}
          {activeWidget.type !== 'table' && 
           activeWidget.type !== 'scatter' && 
           (activeWidget.measureCol || activeWidget.type === 'metric') && (
            <div className="space-y-1.5 pt-2 border-t border-slate-800/60">
              <label className="text-[10px] uppercase font-bold tracking-wider text-slate-500 block">
                Primary Aggregation
              </label>
              <select
                className="w-full bg-slate-950 border border-slate-800 rounded px-2 py-1 text-xs outline-none text-slate-300 cursor-pointer focus:border-indigo-500"
                value={activeWidget.aggregation}
                onChange={(e) => handleFieldChange('aggregation', e.target.value as AggregationFn)}
              >
                {getAggsForColumn(activeWidget.measureCol).map((agg) => (
                  <option key={agg} value={agg}>
                    {AGG_LABELS[agg]}
                  </option>
                ))}
              </select>
            </div>
          )}
        </div>

      </div>

      {/* Auto join plan visual check */}
      <div className="border-t border-slate-800 bg-slate-950/40 p-3 text-[10px] text-slate-500 font-mono space-y-1">
        <div className="text-[9px] uppercase font-bold text-slate-500 tracking-wider">
          Query Planner Status
        </div>
        <div className="flex items-center justify-between text-slate-400">
          <span>Target widget type:</span>
          <span className="text-indigo-400 font-bold uppercase">{activeWidget.type}</span>
        </div>
        {activeWidget.sql ? (
          <div className="text-emerald-400 flex items-center gap-1 font-semibold mt-1">
            <span>✓ Join planning compiles</span>
          </div>
        ) : (
          <div className="text-amber-500 flex items-center gap-1 font-semibold mt-1">
            <span>⚠ Configure fields to compile</span>
          </div>
        )}
      </div>
    </aside>
  );
}
