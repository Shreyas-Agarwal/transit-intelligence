import { useRef, useEffect, useMemo } from 'react';
import ReactECharts from 'echarts-for-react';
import type { ECharts } from 'echarts';

import type { Widget } from '../../../analytics/registry/types';
import { buildChartOptions } from '../../../analytics/charts/builder';
import { useDashboardStore } from '../../../store/dashboardStore';
import { useBenchmarkStore } from '../../../store/benchmarkStore';
import WidgetShell from './WidgetShell';

//-------------------------------------
// Chart Widget
//
// Renders an ECharts chart derived from the widget's query result.
// Responsibilities:
//   - Build EChartsOption from result + widget config (via buildChartOptions)
//   - Emit cross-highlight events on bar/pie click → dashboardStore.setHighlight
//   - React to dashboard-wide highlight state using ECharts dispatchAction API
//   - Measure ECharts render/highlight propagation times
//   - Handle card resize using local ResizeObserver
//-------------------------------------

interface Props {
  widget: Widget;
}

export default function ChartWidget({ widget }: Props) {
  const chartRef = useRef<ReactECharts>(null);
  
  const highlightContext = useDashboardStore((s) => s.highlightContext);
  const theme = useDashboardStore((s) => s.theme);
  const setHighlight = useDashboardStore((s) => s.setHighlight);
  const clearHighlight = useDashboardStore((s) => s.clearHighlight);

  // Track when the most recent render started (for query render time measurement)
  const renderStartRef = useRef<number | null>(null);

  // Mark render start time each time a new result arrives
  useEffect(() => {
    if (!widget.isLoading && widget.result) {
      renderStartRef.current = performance.now();
    }
  }, [widget.result, widget.isLoading]);

  // Derive chart options from the current result
  const chartOptions = useMemo(() => {
    if (!widget.result || !widget.dimensionCol || !widget.measureCol) return null;
    
    // Determine chart type from flat taxonomy
    const chartType = widget.type === 'bar' || widget.type === 'bar-horizontal' || widget.type === 'line' || widget.type === 'pie' || widget.type === 'scatter' 
      ? widget.type 
      : 'bar';

    return buildChartOptions(
      chartType,
      widget.result.rows,
      widget.dimensionCol,
      widget.measureCol,
      theme,
      widget.barStackMode,
    );
  }, [widget.result, widget.type, widget.dimensionCol, widget.measureCol, theme, widget.barStackMode]);

  // Cross-highlight click handler
  const onEvents = useMemo(
    () => ({
      click: (params: { name?: string; value?: unknown }) => {
        const clickedValue = params.name ?? String(params.value ?? '');
        if (clickedValue && widget.dimensionCol) {
          if (highlightContext.column === widget.dimensionCol && highlightContext.value === clickedValue) {
            clearHighlight();
          } else {
            setHighlight({
              column: widget.dimensionCol,
              value: clickedValue,
            });
          }
        }
      },
    }),
    [widget.dimensionCol, highlightContext.column, highlightContext.value, setHighlight, clearHighlight],
  );

  // Capture ECharts initial render time via the chart instance
  const onChartReady = (chart: ECharts) => {
    chart.on('finished', () => {
      if (renderStartRef.current != null) {
        const ms = performance.now() - renderStartRef.current;
        useBenchmarkStore.getState().addEvent({
          eventType: 'chart-render',
          durationMs: ms,
          widgetId: widget.id,
          rowsReturned: widget.result?.rowCount,
        });
        renderStartRef.current = null;
      }
    });
  };

  // Programmatic cross-highlighting
  useEffect(() => {
    const chart = chartRef.current?.getEchartsInstance();
    if (!chart) return;

    // Reset emphasis state first
    chart.dispatchAction({
      type: 'downplay',
      seriesIndex: 0,
    });

    if (highlightContext.value && highlightContext.column === widget.dimensionCol) {
      chart.dispatchAction({
        type: 'highlight',
        seriesIndex: 0,
        name: highlightContext.value,
      });
    }

    if (highlightContext.appliedAt != null) {
      const ms = performance.now() - highlightContext.appliedAt;
      useBenchmarkStore.getState().addEvent({
        eventType: 'cross-highlight',
        durationMs: ms,
        widgetId: widget.id,
        metadata: { column: highlightContext.column, value: highlightContext.value },
      });
    }
  }, [highlightContext, widget.dimensionCol, widget.id]);

  // Trigger ECharts resize when the parent element is resized
  useEffect(() => {
    const chart = chartRef.current?.getEchartsInstance();
    if (!chart) return;
    const parentEl = chart.getDom().parentElement;
    if (!parentEl) return;

    const observer = new ResizeObserver(() => {
      chart.resize();
    });
    observer.observe(parentEl);
    return () => observer.disconnect();
  }, [widget.result]);

  return (
    <WidgetShell widget={widget}>
      {chartOptions && widget.result ? (
        <div className="chart-widget-body" style={{ height: '100%', width: '100%', minHeight: '100%' }}>
          <ReactECharts
            ref={chartRef}
            option={chartOptions}
            style={{ height: '100%', width: '100%' }}
            opts={{ renderer: 'canvas' }}
            onChartReady={onChartReady}
            onEvents={onEvents}
            notMerge
            lazyUpdate={false}
          />
          {widget.result.rowCount === 0 && (
            <p className="widget-no-data">No data returned for this query.</p>
          )}
        </div>
      ) : (
        !widget.isLoading && !widget.error && (
          <p className="widget-no-data">
            Configure a dimension and measure to generate a chart.
          </p>
        )
      )}
    </WidgetShell>
  );
}
