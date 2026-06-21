import type { EChartsOption } from 'echarts';
import type { ChartType } from '../registry/types';

//-------------------------------------
// Analytics Workbench — ECharts Options Builder
//
// Pure function: maps (chartType, rows, dimCol, msrCol, theme) → EChartsOption.
// Updates styling dynamically based on the current dashboard theme.
//-------------------------------------

// ─── Design tokens ────────────────────────────────────────────────────────────

const PALETTE = [
  '#818cf8', // indigo-400
  '#34d399', // emerald-400
  '#f472b6', // pink-400
  '#fb923c', // orange-400
  '#38bdf8', // sky-400
  '#a78bfa', // violet-400
  '#4ade80', // green-400
  '#facc15', // yellow-400
  '#f87171', // red-400
  '#2dd4bf', // teal-400
];

function getTooltipStyle(theme: 'dark' | 'light') {
  return {
    backgroundColor: theme === 'light' ? '#ffffff' : '#1e1f2e',
    borderColor: theme === 'light' ? '#cbd5e1' : '#2d2f45',
    textStyle: { color: theme === 'light' ? '#1e293b' : '#e2e8f0', fontSize: 13 },
  };
}

function getAxisStyle(theme: 'dark' | 'light') {
  return {
    splitLine: { lineStyle: { color: theme === 'light' ? '#f1f5f9' : '#2d2f45', type: 'dashed' as const } },
    axisLabel: { color: theme === 'light' ? '#475569' : '#94a3b8', fontSize: 11 },
    axisLine: { lineStyle: { color: theme === 'light' ? '#cbd5e1' : '#2d2f45' } },
  };
}

function getLegendStyle(theme: 'dark' | 'light') {
  return {
    show: true,
    textStyle: { color: theme === 'light' ? '#475569' : '#e2e8f0', fontSize: 11 },
    top: '4%',
    right: '4%',
    icon: 'circle',
  };
}

// ─── Public API ───────────────────────────────────────────────────────────────

/**
 * Build a complete EChartsOption for the given chart type, data, and active theme.
 *
 * @param chartType - One of the five supported chart types
 * @param rows      - Normalised row array
 * @param dimCol    - Dimension column
 * @param msrCol    - Measure column
 * @param theme     - Current active theme ('dark' | 'light')
 * @returns         - EChartsOption
 */
export function buildChartOptions(
  chartType: ChartType,
  rows: Record<string, unknown>[],
  dimCol: string,
  msrCol: string,
  theme: 'dark' | 'light' = 'dark',
  barStackMode?: 'clustered' | 'stacked' | 'stacked-100'
): EChartsOption {
  switch (chartType) {
    case 'bar':
      return buildBar(rows, dimCol, msrCol, theme, barStackMode);
    case 'bar-horizontal':
      return buildBarHorizontal(rows, dimCol, msrCol, theme, barStackMode);
    case 'pie':
      return buildPie(rows, dimCol, msrCol, theme);
    case 'line':
      return buildLine(rows, dimCol, msrCol, theme);
    case 'scatter':
      return buildScatter(rows, dimCol, msrCol, theme);
  }
}

// ─── Chart builders ───────────────────────────────────────────────────────────

function buildBar(
  rows: Record<string, unknown>[],
  dimCol: string,
  msrCol: string,
  theme: 'dark' | 'light',
  barStackMode: 'clustered' | 'stacked' | 'stacked-100' = 'clustered'
): EChartsOption {
  const dimKey = dimCol.includes('.') ? dimCol.split('.').pop()! : dimCol;
  const msrKey = 'value'; // The planner SELECT alias is always 'value'
  const displayName = msrCol.split('.').pop() || msrCol;

  const tooltipBase = getTooltipStyle(theme);
  const axisStyle = getAxisStyle(theme);
  const legendStyle = getLegendStyle(theme);

  const hasLegend = rows.length > 0 && 'legend' in rows[0];

  let categories: string[];
  let series: Record<string, unknown>[];
  let colorPalette: string[];

  if (hasLegend) {
    categories = Array.from(new Set(rows.map((r) => String(r[dimKey] ?? ''))));
    const legendValues = Array.from(new Set(rows.map((r) => String(r['legend'] ?? ''))));
    colorPalette = PALETTE;

    // Pre-calculate column totals if stack-100 is selected
    const categoryTotals: Record<string, number> = {};
    if (barStackMode === 'stacked-100') {
      categories.forEach((cat) => {
        let sum = 0;
        legendValues.forEach((legVal) => {
          const match = rows.find(
            (r) => String(r[dimKey] ?? '') === cat && String(r['legend'] ?? '') === legVal
          );
          sum += match ? Number(match[msrKey] ?? 0) : 0;
        });
        categoryTotals[cat] = sum;
      });
    }

    series = legendValues.map((legVal) => {
      const seriesData = categories.map((cat) => {
        const match = rows.find((r) => String(r[dimKey] ?? '') === cat && String(r['legend'] ?? '') === legVal);
        const val = match ? Number(match[msrKey] ?? 0) : 0;
        if (barStackMode === 'stacked-100') {
          const total = categoryTotals[cat];
          return total > 0 ? Number(((val / total) * 100).toFixed(2)) : 0;
        }
        return val;
      });

      const seriesObj: Record<string, unknown> = {
        name: legVal,
        type: 'bar',
        data: seriesData,
        barMaxWidth: 32,
        itemStyle: { borderRadius: barStackMode !== 'clustered' ? 0 : [4, 4, 0, 0] },
        emphasis: { focus: 'self' },
      };

      if (barStackMode !== 'clustered') {
        seriesObj.stack = 'total';
      }

      return seriesObj;
    });
  } else {
    categories = rows.map((r) => String(r[dimKey] ?? ''));
    const values = rows.map((r) => Number(r[msrKey] ?? 0));
    colorPalette = [PALETTE[0]];

    series = [
      {
        name: displayName,
        type: 'bar',
        data: values,
        barMaxWidth: 48,
        itemStyle: {
          borderRadius: [4, 4, 0, 0],
          color: {
            type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: '#818cf8' },
              { offset: 1, color: '#6366f1' },
            ],
          },
        },
        emphasis: { focus: 'self', itemStyle: { color: '#a5b4fc' } },
      },
    ];
  }

  return {
    color: colorPalette,
    tooltip: { 
      ...tooltipBase, 
      trigger: 'axis',
      valueFormatter: barStackMode === 'stacked-100' 
        ? (value: unknown) => `${Number(value).toFixed(1)}%` 
        : undefined
    },
    legend: legendStyle,
    grid: { left: '2%', right: '3%', bottom: '8%', top: '15%', containLabel: true },
    xAxis: {
      type: 'category',
      data: categories,
      ...axisStyle,
      axisLabel: {
        ...axisStyle.axisLabel,
        rotate: categories.length > 8 ? 35 : 0,
        overflow: 'truncate',
        width: 80,
      },
    },
    yAxis: {
      type: 'value',
      name: barStackMode === 'stacked-100' ? 'Percentage' : displayName,
      nameTextStyle: { color: theme === 'light' ? '#475569' : '#64748b', fontSize: 11 },
      min: barStackMode === 'stacked-100' ? 0 : undefined,
      max: barStackMode === 'stacked-100' ? 100 : undefined,
      axisLabel: {
        formatter: barStackMode === 'stacked-100' ? '{value}%' : undefined,
      },
      ...axisStyle,
    },
    series,
  };
}

function buildBarHorizontal(
  rows: Record<string, unknown>[],
  dimCol: string,
  msrCol: string,
  theme: 'dark' | 'light',
  barStackMode: 'clustered' | 'stacked' | 'stacked-100' = 'clustered'
): EChartsOption {
  const dimKey = dimCol.includes('.') ? dimCol.split('.').pop()! : dimCol;
  const msrKey = 'value';
  const displayName = msrCol.split('.').pop() || msrCol;

  const tooltipBase = getTooltipStyle(theme);
  const axisStyle = getAxisStyle(theme);
  const legendStyle = getLegendStyle(theme);

  const hasLegend = rows.length > 0 && 'legend' in rows[0];

  let categories: string[];
  let series: Record<string, unknown>[];
  let colorPalette: string[];

  if (hasLegend) {
    categories = Array.from(new Set(rows.map((r) => String(r[dimKey] ?? ''))));
    const legendValues = Array.from(new Set(rows.map((r) => String(r['legend'] ?? ''))));
    colorPalette = PALETTE;

    // Pre-calculate column totals if stack-100 is selected
    const categoryTotals: Record<string, number> = {};
    if (barStackMode === 'stacked-100') {
      categories.forEach((cat) => {
        let sum = 0;
        legendValues.forEach((legVal) => {
          const match = rows.find(
            (r) => String(r[dimKey] ?? '') === cat && String(r['legend'] ?? '') === legVal
          );
          sum += match ? Number(match[msrKey] ?? 0) : 0;
        });
        categoryTotals[cat] = sum;
      });
    }

    series = legendValues.map((legVal) => {
      const seriesData = categories.map((cat) => {
        const match = rows.find((r) => String(r[dimKey] ?? '') === cat && String(r['legend'] ?? '') === legVal);
        const val = match ? Number(match[msrKey] ?? 0) : 0;
        if (barStackMode === 'stacked-100') {
          const total = categoryTotals[cat];
          return total > 0 ? Number(((val / total) * 100).toFixed(2)) : 0;
        }
        return val;
      });

      const seriesObj: Record<string, unknown> = {
        name: legVal,
        type: 'bar',
        data: [...seriesData].reverse(),
        barMaxWidth: 24,
        itemStyle: { borderRadius: barStackMode !== 'clustered' ? 0 : [0, 4, 4, 0] },
        emphasis: { focus: 'self' },
      };

      if (barStackMode !== 'clustered') {
        seriesObj.stack = 'total';
      }

      return seriesObj;
    });
    categories = [...categories].reverse();
  } else {
    const rawCategories = rows.map((r) => String(r[dimKey] ?? ''));
    const values = rows.map((r) => Number(r[msrKey] ?? 0));
    categories = [...rawCategories].reverse();
    colorPalette = [PALETTE[1]];

    series = [
      {
        name: displayName,
        type: 'bar',
        data: [...values].reverse(),
        barMaxWidth: 28,
        itemStyle: {
          borderRadius: [0, 4, 4, 0],
          color: {
            type: 'linear', x: 0, y: 0, x2: 1, y2: 0,
            colorStops: [
              { offset: 0, color: '#059669' },
              { offset: 1, color: '#34d399' },
            ],
          },
        },
        emphasis: { focus: 'self', itemStyle: { color: '#6ee7b7' } },
      },
    ];
  }

  return {
    color: colorPalette,
    tooltip: { 
      ...tooltipBase, 
      trigger: 'axis',
      valueFormatter: barStackMode === 'stacked-100' 
        ? (value: unknown) => `${Number(value).toFixed(1)}%` 
        : undefined
    },
    legend: legendStyle,
    grid: { left: '2%', right: '4%', bottom: '3%', top: '15%', containLabel: true },
    xAxis: {
      type: 'value',
      name: barStackMode === 'stacked-100' ? 'Percentage' : displayName,
      nameTextStyle: { color: theme === 'light' ? '#475569' : '#64748b', fontSize: 11 },
      min: barStackMode === 'stacked-100' ? 0 : undefined,
      max: barStackMode === 'stacked-100' ? 100 : undefined,
      axisLabel: {
        formatter: barStackMode === 'stacked-100' ? '{value}%' : undefined,
      },
      ...axisStyle,
    },
    yAxis: {
      type: 'category',
      data: categories,
      ...axisStyle,
      axisLabel: {
        ...axisStyle.axisLabel,
        overflow: 'truncate',
        width: 110,
      },
    },
    series,
  };
}

function buildPie(
  rows: Record<string, unknown>[],
  dimCol: string,
  msrCol: string,
  theme: 'dark' | 'light'
): EChartsOption {
  const dimKey = dimCol.includes('.') ? dimCol.split('.').pop()! : dimCol;
  const msrKey = 'value';

  const data = rows.map((r) => ({
    name: String(r[dimKey] ?? ''),
    value: Number(r[msrKey] ?? 0),
  }));
  
  const tooltipBase = getTooltipStyle(theme);
  const displayName = msrCol.split('.').pop() || msrCol;

  return {
    color: PALETTE,
    tooltip: {
      ...tooltipBase,
      trigger: 'item',
      formatter: `{b}<br/>${displayName}: <strong>{c}</strong> ({d}%)`,
    },
    legend: {
      show: true,
      orient: 'vertical',
      right: '4%',
      top: 'center',
      textStyle: { color: theme === 'light' ? '#475569' : '#e2e8f0', fontSize: 11 },
      formatter: (n: string) => (n.length > 20 ? n.slice(0, 18) + '…' : n),
    },
    series: [
      {
        name: displayName,
        type: 'pie',
        radius: ['40%', '68%'],
        center: ['38%', '50%'],
        avoidLabelOverlap: true,
        itemStyle: { borderRadius: 6, borderColor: theme === 'light' ? '#ffffff' : '#0f1019', borderWidth: 2 },
        label: { show: false },
        emphasis: {
          focus: 'self',
          label: { show: true, fontSize: 13, fontWeight: 'bold', color: theme === 'light' ? '#1e293b' : '#e2e8f0' },
        },
        data,
      },
    ],
  };
}

function buildLine(
  rows: Record<string, unknown>[],
  dimCol: string,
  msrCol: string,
  theme: 'dark' | 'light'
): EChartsOption {
  const dimKey = dimCol.includes('.') ? dimCol.split('.').pop()! : dimCol;
  const msrKey = 'value';
  const displayName = msrCol.split('.').pop() || msrCol;

  const tooltipBase = getTooltipStyle(theme);
  const axisStyle = getAxisStyle(theme);
  const legendStyle = getLegendStyle(theme);

  const hasLegend = rows.length > 0 && 'legend' in rows[0];

  let categories: string[];
  let series: Record<string, unknown>[];
  let colorPalette: string[];

  if (hasLegend) {
    categories = Array.from(new Set(rows.map((r) => String(r[dimKey] ?? ''))));
    const legendValues = Array.from(new Set(rows.map((r) => String(r['legend'] ?? ''))));
    colorPalette = PALETTE;

    series = legendValues.map((legVal) => {
      const seriesData = categories.map((cat) => {
        const match = rows.find((r) => String(r[dimKey] ?? '') === cat && String(r['legend'] ?? '') === legVal);
        return match ? Number(match[msrKey] ?? 0) : 0;
      });

      return {
        name: legVal,
        type: 'line',
        data: seriesData,
        smooth: true,
        symbol: 'circle',
        symbolSize: 5,
        emphasis: { focus: 'self', scale: true },
      };
    });
  } else {
    categories = rows.map((r) => String(r[dimKey] ?? ''));
    const values = rows.map((r) => Number(r[msrKey] ?? 0));
    colorPalette = [PALETTE[0]];

    series = [
      {
        name: displayName,
        type: 'line',
        data: values,
        smooth: true,
        symbol: 'circle',
        symbolSize: 5,
        lineStyle: { color: '#818cf8', width: 2 },
        areaStyle: {
          color: {
            type: 'linear', x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: 'rgba(129,140,248,0.22)' },
              { offset: 1, color: 'rgba(129,140,248,0)' },
            ],
          },
        },
        itemStyle: { color: '#a5b4fc' },
        emphasis: { focus: 'self', scale: true },
      },
    ];
  }

  return {
    color: colorPalette,
    tooltip: { ...tooltipBase, trigger: 'axis' },
    legend: legendStyle,
    grid: { left: '2%', right: '3%', bottom: '8%', top: '15%', containLabel: true },
    xAxis: {
      type: 'category',
      data: categories,
      boundaryGap: false,
      ...axisStyle,
      axisLabel: {
        ...axisStyle.axisLabel,
        rotate: categories.length > 8 ? 35 : 0,
        overflow: 'truncate',
        width: 80,
      },
    },
    yAxis: {
      type: 'value',
      name: displayName,
      nameTextStyle: { color: theme === 'light' ? '#475569' : '#64748b', fontSize: 11 },
      ...axisStyle,
    },
    series,
  };
}

function buildScatter(
  rows: Record<string, unknown>[],
  xCol: string,
  yCol: string,
  theme: 'dark' | 'light'
): EChartsOption {
  // Scatter planner SQL returns columns specifically named "x" and "y"
  const data = rows.map((r) => [Number(r['x'] ?? 0), Number(r['y'] ?? 0)]);
  
  const tooltipBase = getTooltipStyle(theme);
  const axisStyle = getAxisStyle(theme);
  const legendStyle = getLegendStyle(theme);
  
  const displayNameX = xCol.split('.').pop() || xCol;
  const displayNameY = yCol.split('.').pop() || yCol;

  return {
    color: [PALETTE[2]],
    tooltip: {
      ...tooltipBase,
      trigger: 'item',
      formatter: `${displayNameX}: <strong>{@[0]}</strong><br/>${displayNameY}: <strong>{@[1]}</strong>`,
    },
    legend: legendStyle,
    grid: { left: '3%', right: '4%', bottom: '8%', top: '15%', containLabel: true },
    xAxis: {
      type: 'value',
      name: displayNameX,
      nameTextStyle: { color: theme === 'light' ? '#475569' : '#64748b', fontSize: 11 },
      ...axisStyle,
    },
    yAxis: {
      type: 'value',
      name: displayNameY,
      nameTextStyle: { color: theme === 'light' ? '#475569' : '#64748b', fontSize: 11 },
      ...axisStyle,
    },
    series: [
      {
        name: `${displayNameX} vs ${displayNameY}`,
        type: 'scatter',
        data,
        symbolSize: 7,
        itemStyle: {
          color: '#f472b6',
          opacity: 0.75,
          borderColor: theme === 'light' ? '#ffffff' : '#0f1019',
          borderWidth: 1,
        },
        emphasis: { focus: 'self', itemStyle: { opacity: 1, color: '#fb7185' } },
      },
    ],
  };
}
