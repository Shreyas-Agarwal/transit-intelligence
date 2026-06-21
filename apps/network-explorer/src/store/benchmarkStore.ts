import { create } from 'zustand';
import type { BenchmarkEvent, BenchmarkSummary } from '../analytics/registry/types';

//-------------------------------------
// Unified Benchmarking Store
//
// Single source of truth for telemetry across the entire Analytics Studio.
// Captures and aggregates all execution, compile, rendering, and interaction metrics.
// Stores metrics in-memory for the current session.
//-------------------------------------

interface BenchmarkState {
  events: BenchmarkEvent[];
  addEvent(event: Omit<BenchmarkEvent, 'id' | 'timestamp'>): void;
  clearEvents(): void;
  getSummary(): BenchmarkSummary;
}

export const useBenchmarkStore = create<BenchmarkState>((set, get) => ({
  events: [],

  addEvent(event) {
    const newEvent: BenchmarkEvent = {
      ...event,
      id: crypto.randomUUID(),
      timestamp: Date.now(),
    };
    set((state) => ({
      events: [newEvent, ...state.events].slice(0, 1000), // retain last 1000 events
    }));
  },

  clearEvents() {
    set({ events: [] });
  },

  getSummary() {
    const { events } = get();

    // 1. Dashboard Load Time
    const loadEvent = events.find((e) => e.eventType === 'dashboard-load');
    const dashboardLoadMs = loadEvent?.durationMs ?? 0;

    // 2. Query execution metrics
    const queryEvents = events.filter((e) => e.eventType === 'query-execution');
    const totalQueries = queryEvents.length;
    const totalRows = queryEvents.reduce((sum, e) => sum + (e.rowsReturned ?? 0), 0);
    const avgQueryMs =
      totalQueries > 0
        ? queryEvents.reduce((sum, e) => sum + e.durationMs, 0) / totalQueries
        : 0;

    const slowestQueryMs =
      totalQueries > 0 ? Math.max(...queryEvents.map((e) => e.durationMs)) : 0;

    const largestResult =
      totalQueries > 0 ? Math.max(...queryEvents.map((e) => e.rowsReturned ?? 0)) : 0;

    // 95th percentile query time
    let pct95QueryMs = 0;
    if (totalQueries > 0) {
      const sortedQueries = [...queryEvents].sort((a, b) => a.durationMs - b.durationMs);
      const idx = Math.floor(sortedQueries.length * 0.95);
      pct95QueryMs = sortedQueries[idx]?.durationMs ?? 0;
    }

    // 3. Render metrics (chart, table, matrix, map render events)
    const renderEvents = events.filter((e) =>
      e.eventType.endsWith('-render')
    );
    const avgRenderMs =
      renderEvents.length > 0
        ? renderEvents.reduce((sum, e) => sum + e.durationMs, 0) / renderEvents.length
        : 0;

    // 4. Highlight metrics
    const highlightEvents = events.filter((e) => e.eventType === 'cross-highlight');
    const avgHighlightMs =
      highlightEvents.length > 0
        ? highlightEvents.reduce((sum, e) => sum + e.durationMs, 0) / highlightEvents.length
        : 0;

    // 5. Most expensive widget (cumulative query + render times)
    const widgetCosts: Record<string, number> = {};
    for (const e of events) {
      if (e.widgetId) {
        widgetCosts[e.widgetId] = (widgetCosts[e.widgetId] || 0) + e.durationMs;
      }
    }
    let mostExpensiveWidgetId: string | null = null;
    let maxCost = 0;
    for (const [wId, cost] of Object.entries(widgetCosts)) {
      if (cost > maxCost) {
        maxCost = cost;
        mostExpensiveWidgetId = wId;
      }
    }

    return {
      dashboardLoadMs,
      totalQueries,
      totalRows,
      avgQueryMs,
      avgRenderMs,
      avgHighlightMs,
      pct95QueryMs,
      largestResult,
      slowestQueryMs,
      mostExpensiveWidgetId,
    };
  },
}));
