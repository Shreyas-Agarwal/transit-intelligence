import type { BenchmarkResult } from '../benchmarks/types';

interface Props {
  results: BenchmarkResult[];
}

export default function BenchmarkTable({ results }: Props) {
  return (
    <table className="min-w-full border">
      <thead>
        <tr>
          <th>Query</th>
          <th>Cold (ms)</th>
          <th>Warm (ms)</th>
        </tr>
      </thead>

      <tbody>
        {results.map((result) => (
          <tr key={result.name}>
            <td>{result.name}</td>
            <td>{result.coldMs.toFixed(2)}</td>
            <td>{result.warmMs.toFixed(2)}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
