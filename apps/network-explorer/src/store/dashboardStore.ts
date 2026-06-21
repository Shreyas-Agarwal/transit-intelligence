import { create } from 'zustand';

import type {
  Widget,
  WidgetConfig,
  FilterContext,
  HighlightContext,
  DiscoveredTable,
} from '../analytics/registry/types';
import { discoverCatalog } from '../analytics/discovery/catalogService';
import { planWidgetSql } from '../analytics/semantic/planner';
import { executeAnalyticsQuery } from '../analytics/services/queryService';
import { getSharedConnection } from '../lib/connection';
import { useBenchmarkStore } from './benchmarkStore';

//-------------------------------------
// Analytics Workbench — Dashboard Zustand Store (Productized)
//
// Single source of truth for dashboard state and actions:
//   - Discovered database catalog schema
//   - Materialized workspace widget configurations
//   - Active SQL filter context and client-side highlight context
//
// Direct interface for in-place configuration edits, widget duplications,
// and automatic telemetry reporting to the central benchmarkStore.
//-------------------------------------

const EMPTY_FILTER: FilterContext = {
  tableName: null,
  column: null,
  value: null,
  appliedAt: null,
};

const EMPTY_HIGHLIGHT: HighlightContext = {
  column: null,
  value: null,
  appliedAt: null,
};

interface DashboardState {
  // ── State ────────────────────────────────────────────────────
  catalog: DiscoveredTable[];
  catalogLoading: boolean;
  catalogError: string | null;
  widgets: Widget[];
  filterContext: FilterContext;
  highlightContext: HighlightContext;
  theme: 'dark' | 'light';
  selectedWidgetId: string | null;

  // ── Actions ──────────────────────────────────────────────────
  initCatalog(): Promise<void>;
  addWidget(config: WidgetConfig): void;
  updateWidget(id: string, patch: Partial<WidgetConfig>): void;
  duplicateWidget(id: string): void;
  removeWidget(id: string): void;
  setSelectedWidgetId(id: string | null): void;
  runWidget(id: string): Promise<void>;
  runAllWidgets(): Promise<void>;
  setFilter(ctx: Omit<FilterContext, 'appliedAt'>): void;
  clearFilter(): void;
  setHighlight(ctx: Omit<HighlightContext, 'appliedAt'>): void;
  clearHighlight(): void;
  setTheme(theme: 'dark' | 'light'): void;
}

export const useDashboardStore = create<DashboardState>((set, get) => ({
  catalog: [],
  catalogLoading: false,
  catalogError: null,
  widgets: [],
  filterContext: EMPTY_FILTER,
  highlightContext: EMPTY_HIGHLIGHT,
  theme: 'dark',
  selectedWidgetId: null,

  // ── Schema Discovery ──────────────────────────────────────────

  async initCatalog() {
    set({ catalogLoading: true, catalogError: null });
    const start = performance.now();
    try {
      const conn = getSharedConnection();
      const catalog = await discoverCatalog(conn);
      set({ catalog, catalogLoading: false });

      useBenchmarkStore.getState().addEvent({
        eventType: 'schema-discovery',
        durationMs: performance.now() - start,
      });
    } catch (err) {
      set({
        catalogError:
          err instanceof Error ? err.message : 'Catalog discovery failed.',
        catalogLoading: false,
      });
    }
  },

  // ── Widget Management ──────────────────────────────────────────

  addWidget(config: WidgetConfig) {
    const id = crypto.randomUUID();
    const newWidget: Widget = {
      ...config,
      id,
      sql: '',
      result: null,
      renderMs: null,
      isLoading: false,
      error: null,
    };
    set((state) => ({ 
      widgets: [...state.widgets, newWidget],
      selectedWidgetId: id,
    }));

    useBenchmarkStore.getState().addEvent({
      eventType: 'widget-creation',
      durationMs: 0,
      widgetId: id,
    });

    void get().runWidget(id);
  },

  updateWidget(id, patch) {
    set((state) => ({
      widgets: state.widgets.map((w) => (w.id === id ? { ...w, ...patch } : w)),
    }));

    useBenchmarkStore.getState().addEvent({
      eventType: 'widget-update',
      durationMs: 0,
      widgetId: id,
      metadata: patch as Record<string, unknown>,
    });

    void get().runWidget(id);
  },

  duplicateWidget(id) {
    const widget = get().widgets.find((w) => w.id === id);
    if (!widget) return;

    const copyId = crypto.randomUUID();
    const copy: Widget = {
      ...widget,
      id: copyId,
      title: `${widget.title} (Copy)`,
      result: null,
      isLoading: false,
      error: null,
    };

    set((state) => ({ 
      widgets: [...state.widgets, copy],
      selectedWidgetId: copyId,
    }));

    useBenchmarkStore.getState().addEvent({
      eventType: 'widget-creation',
      durationMs: 0,
      widgetId: copyId,
      metadata: { duplicatedFrom: id },
    });

    void get().runWidget(copyId);
  },

  removeWidget(id: string) {
    set((state) => ({
      widgets: state.widgets.filter((w) => w.id !== id),
      selectedWidgetId: state.selectedWidgetId === id ? null : state.selectedWidgetId,
    }));

    useBenchmarkStore.getState().addEvent({
      eventType: 'widget-deletion',
      durationMs: 0,
      widgetId: id,
    });
  },

  setSelectedWidgetId(id) {
    set({ selectedWidgetId: id });
  },

  // ── Query Compilation & Run ───────────────────────────────────

  async runWidget(id: string) {
    const { widgets, filterContext, catalog } = get();
    const widget = widgets.find((w) => w.id === id);
    if (!widget) return;

    _updateWidget(set, id, { isLoading: true, error: null });

    try {
      // 1. Join Planning (SQL Compilation)
      const compileStart = performance.now();
      const sql = planWidgetSql(widget, filterContext, catalog);
      const joinMs = performance.now() - compileStart;

      useBenchmarkStore.getState().addEvent({
        eventType: 'join-planning',
        durationMs: joinMs,
        widgetId: id,
        metadata: { sql },
      });

      // 2. Query execution in DuckDB WASM
      const queryStart = performance.now();
      const conn = getSharedConnection();
      const result = await executeAnalyticsQuery(sql, conn);
      const queryMs = performance.now() - queryStart;

      useBenchmarkStore.getState().addEvent({
        eventType: 'query-execution',
        durationMs: queryMs,
        widgetId: id,
        rowsReturned: result.rowCount,
        metadata: { sql },
      });

      _updateWidget(set, id, { sql, result, isLoading: false, error: null });
    } catch (err) {
      _updateWidget(set, id, {
        error: err instanceof Error ? err.message : 'Query failed.',
        isLoading: false,
      });
    }
  },

  async runAllWidgets() {
    const { widgets } = get();
    await Promise.all(widgets.map((w) => get().runWidget(w.id)));
  },

  // ── SQL Cross-filtering (Context updates) ──────────────────────

  setFilter(ctx: Omit<FilterContext, 'appliedAt'>) {
    const appliedAt = performance.now();
    set({ filterContext: { ...ctx, appliedAt } });
    void get().runAllWidgets();
  },

  clearFilter() {
    set({ filterContext: EMPTY_FILTER });
    void get().runAllWidgets();
  },

  // ── Client-side Highlights ────────────────────────────────────

  setHighlight(ctx: Omit<HighlightContext, 'appliedAt'>) {
    const appliedAt = performance.now();
    set({ highlightContext: { ...ctx, appliedAt } });
  },

  clearHighlight() {
    set({ highlightContext: EMPTY_HIGHLIGHT });
  },

  // ── Color Theme ───────────────────────────────────────────────

  setTheme(theme) {
    set({ theme });
  },
}));

// ─── Private helpers ──────────────────────────────────────────────────────────

type SetFn = (
  partial: Partial<DashboardState> | ((state: DashboardState) => Partial<DashboardState>),
) => void;

function _updateWidget(set: SetFn, id: string, patch: Partial<Widget>) {
  set((state) => ({
    widgets: state.widgets.map((w) => (w.id === id ? { ...w, ...patch } : w)),
  }));
}
