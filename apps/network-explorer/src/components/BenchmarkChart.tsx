import ReactECharts from 'echarts-for-react';
import type { BenchmarkResult } from '../benchmarks/types';

interface Props {
  title: string;
  results: BenchmarkResult[];
}

export default function BenchmarkChart({ title, results }: Props) {
  const option = {
    title: {
      text: title,
    },

    tooltip: {
      trigger: 'axis',
    },

    legend: {
      data: ['Cold', 'Warm'],
    },

    xAxis: {
      type: 'category',
      data: results.map((r) => r.name),
    },

    yAxis: {
      type: 'value',
      name: 'ms',
    },

    series: [
      {
        name: 'Cold',
        type: 'bar',
        data: results.map((r) => r.coldMs),
      },
      {
        name: 'Warm',
        type: 'bar',
        data: results.map((r) => r.warmMs),
      },
    ],
  };

  return (
    <ReactECharts
      option={option}
      style={{
        height: 400,
      }}
    />
  );
}
