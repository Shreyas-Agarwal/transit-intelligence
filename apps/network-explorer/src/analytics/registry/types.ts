//-------------------------------------
// Analytics Workbench — Core Type Definitions
//
// Single source of truth for all types across the analytics layer.
// React components, services, the SQL generator, and the chart builder all
// import from here. Nothing in this file imports from our own codebase.
//-------------------------------------

// ─── Chart Types ────────────────────────────────────────────────────────────

/**
 * All supported ECharts visualisation types.
 * Scatter requires two numeric columns (dimCol = x, msrCol = y).
 */
export type ChartType = 'bar' | 'bar-horizontal' | 'pie' | 'line' | 'scatter';

// ─── Legacy Registry Types (kept for dataset.ts compatibility shim) ──────────

/** @deprecated Use DiscoveredColumn instead for discovery-driven analytics. */
export interface DimensionDef {
  id: string;
  label: string;
  column: string;
}

/** @deprecated Use DiscoveredColumn instead for discovery-driven analytics. */
export interface MeasureDef {
  id: string;
  label: string;
  column: string;
  aggregation: 'SUM' | 'AVG' | 'COUNT' | 'none';
}

/** @deprecated Use DiscoveredTable instead for discovery-driven analytics. */
export interface DatasetDef {
  id: string;
  label: string;
  description: string;
  viewName: string;
  dimensions: DimensionDef[];
  measures: MeasureDef[];
  defaultChartType: ChartType;
  defaultDimensionId: string;
  defaultMeasureId: string;
}

// ─── Query Result ────────────────────────────────────────────────────────────

/**
 * Normalised result from a single DuckDB query execution.
 * Produced by `executeAnalyticsQuery` in the query service.
 */
export interface QueryResult {
  /** Plain JS object rows (column name → value). */
  rows: Record<string, unknown>[];
  /** Wall-clock execution time in milliseconds. */
  executionMs: number;
  /** Number of rows returned. */
  rowCount: number;
}

// ─── Schema Discovery ────────────────────────────────────────────────────────

/**
 * Classification of a discovered column, derived from its SQL data type
 * and name heuristics.
 */
export type ColumnKind = 'dimension' | 'measure' | 'geo' | 'id';

/** A single column in a discovered DuckDB table or view. */
export interface DiscoveredColumn {
  /** Column name as returned by information_schema. */
  name: string;
  /** SQL data type string (e.g. VARCHAR, BIGINT, DOUBLE). */
  dataType: string;
  /** Automatically classified role for this column. */
  kind: ColumnKind;
}

/** A table or view visible in the DuckDB catalog. */
export interface DiscoveredTable {
  /** Table/view name as returned by information_schema. */
  name: string;
  /** All columns for this table, classified. */
  columns: DiscoveredColumn[];
}

/** Flattened taxonomy of first-class widget types. */
export type WidgetType =
  | 'metric'
  | 'table'
  | 'matrix'
  | 'map'
  | 'bar'
  | 'bar-horizontal'
  | 'line'
  | 'pie'
  | 'scatter';

/** Supported aggregation functions for measures. */
export type AggregationFn =
  | 'COUNT'
  | 'COUNT_DISTINCT'
  | 'SUM'
  | 'AVG'
  | 'MIN'
  | 'MAX'
  | 'none';

/** Configuration for custom table columns, allowing multi-table query building. */
export interface TableColumnConfig {
  name: string;
  aggregation: AggregationFn;
  alias?: string;
}

/**
 * Configuration input when creating a new widget.
 * The dashboard store derives `id`, `sql`, `result`, and runtime state.
 */
export interface WidgetConfig {
  type: WidgetType;
  title: string;
  tableName: string;
  dimensionCol: string | null;
  measureCol: string | null;
  aggregation: AggregationFn;
  chartType?: ChartType; // kept for legacy compatibility

  // Matrix widget dimensions
  matrixRowCol?: string | null;
  matrixColCol?: string | null;

  // Custom columns for the enhanced Table widget
  tableColumns?: TableColumnConfig[];

  // Map settings
  mapPinSize?: number;

  // Legend/Color grouping column
  legendCol?: string | null;

  // Bar chart stack mode: 'clustered' | 'stacked' | 'stacked-100'
  barStackMode?: 'clustered' | 'stacked' | 'stacked-100';

  // Widget custom board sizing (in pixels)
  width?: number;
  height?: number;
}

/**
 * A fully materialised widget in the dashboard store.
 * Contains both configuration and runtime state.
 */
export interface Widget extends WidgetConfig {
  /** Unique widget identifier (crypto.randomUUID). */
  id: string;
  /** Generated SQL for the current configuration + active filter. */
  sql: string;
  /** Latest query result, null if never executed. */
  result: QueryResult | null;
  /** ECharts / Map / Matrix render time in ms, measured after data arrives. */
  renderMs: number | null;
  /** Whether a query is currently in-flight. */
  isLoading: boolean;
  /** Error message if the last query failed. */
  error: string | null;
}

// ─── Cross-Filter / Cross-Highlighting ───────────────────────────────────────

/**
 * Dashboard-wide active filter context (for SQL queries).
 */
export interface FilterContext {
  /** Source table that emitted the filter. Null = no active filter. */
  tableName: string | null;
  /** Column name that was clicked. */
  column: string | null;
  /** The clicked value (used in WHERE clause). */
  value: string | null;
  /**
   * `performance.now()` timestamp captured when setFilter was called.
   * Used to measure cross-filter propagation time.
   */
  appliedAt: number | null;
}

/**
 * Dashboard-wide active highlight context (client-side rendering only).
 */
export interface HighlightContext {
  /** Column name that is highlighted. */
  column: string | null;
  /** The highlighted value (dim others, highlight matching). */
  value: string | null;
  /**
   * `performance.now()` timestamp captured when setHighlight was called.
   * Used to measure cross-highlight propagation time.
   */
  appliedAt: number | null;
}

// ─── Unified Benchmarking Model ──────────────────────────────────────────────

export type BenchmarkEventType =
  | 'dashboard-load'
  | 'schema-discovery'
  | 'join-planning'
  | 'query-execution'
  | 'cross-highlight'
  | 'map-render'
  | 'table-render'
  | 'matrix-render'
  | 'chart-render'
  | 'widget-creation'
  | 'widget-update'
  | 'widget-deletion';

export interface BenchmarkEvent {
  id: string;
  timestamp: number;
  eventType: BenchmarkEventType;
  durationMs: number;
  widgetId?: string;
  rowsReturned?: number;
  metadata?: Record<string, unknown>;
}

export interface BenchmarkSummary {
  dashboardLoadMs: number;
  totalQueries: number;
  totalRows: number;
  avgQueryMs: number;
  avgRenderMs: number;
  avgHighlightMs: number;
  pct95QueryMs: number;
  largestResult: number;
  slowestQueryMs: number;
  mostExpensiveWidgetId: string | null;
}

// ─── Diagnostics ─────────────────────────────────────────────────────────────

/**
 * One diagnostic record per widget query execution.
 * Accumulated in the dashboard store and displayed in the DiagnosticsPanel.
 */
export interface DiagnosticEntry {
  widgetId: string;
  widgetTitle: string;
  sql: string;
  executionMs: number;
  /** Time for ECharts or other widgets to finish rendering after data arrived. */
  renderMs: number | null;
  rowCount: number;
  /** True if a filter was active when this query ran. */
  filterApplied: boolean;
  /**
   * Total time from when the filter was set to when this widget's query
   * completed. Null for unfiltered runs.
   */
  propagationMs: number | null;
  /** Time for highlighting changes to propagate and render. */
  highlightPropagationMs?: number | null;
  /** Time taken to compile and resolve joins for the multi-table query. */
  joinPlanningMs?: number | null;
  /** performance.now() timestamp when this entry was recorded. */
  timestamp: number;
}
