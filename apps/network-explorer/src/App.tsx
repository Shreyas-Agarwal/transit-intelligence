import { useEffect, useState } from 'react';

import { getDuckDB, registerParquetFile } from './lib/duckdb';
import { setSharedConnection } from './lib/connection';
import AnalyticsStudio from './components/AnalyticsStudio';

function App() {
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function initialize() {
      try {
        // Initialize DuckDB connection
        const db = await getDuckDB();
        const conn = await db.connect();

        // Register Parquet files in memory
        await registerParquetFile(db, 'zurich_stops.parquet');
        await registerParquetFile(db, 'zurich_routes.parquet');
        await registerParquetFile(db, 'zurich_trips.parquet');
        await registerParquetFile(db, 'zurich_agencies.parquet');
        await registerParquetFile(db, 'zurich_stop_times.parquet');

        // Create core GTFS Views
        await conn.query(`
          CREATE OR REPLACE VIEW stops AS
          SELECT * FROM read_parquet('zurich_stops.parquet')
        `);

        await conn.query(`
          CREATE OR REPLACE VIEW routes AS
          SELECT * FROM read_parquet('zurich_routes.parquet')
        `);

        await conn.query(`
          CREATE OR REPLACE VIEW trips AS
          SELECT * FROM read_parquet('zurich_trips.parquet')
        `);

        await conn.query(`
          CREATE OR REPLACE VIEW agencies AS
          SELECT * FROM read_parquet('zurich_agencies.parquet')
        `);

        await conn.query(`
          CREATE OR REPLACE VIEW stop_times AS
          SELECT * FROM read_parquet('zurich_stop_times.parquet')
        `);

        // Create pre-aggregated views for dashboard convenience
        await conn.query(`
          CREATE OR REPLACE VIEW agency_summary AS
          SELECT
            a.agency_name,
            COUNT(DISTINCT r.route_id) AS route_count,
            COUNT(DISTINCT t.trip_id) AS trip_count
          FROM agencies a
          JOIN routes r ON a.agency_id = r.agency_id
          JOIN trips t ON r.route_id = t.route_id
          GROUP BY a.agency_name
          ORDER BY trip_count DESC
        `);

        await conn.query(`
          CREATE OR REPLACE VIEW route_summary AS
          SELECT
            r.route_id,
            r.route_short_name,
            COUNT(t.trip_id) AS trip_count
          FROM routes r
          JOIN trips t ON r.route_id = t.route_id
          GROUP BY r.route_id, r.route_short_name
          ORDER BY trip_count DESC
        `);

        await conn.query(`
          CREATE OR REPLACE VIEW network_summary AS
          SELECT 'Total Stops'        AS metric_name, COUNT(*)::BIGINT AS metric_value FROM stops
          UNION ALL
          SELECT 'Total Routes',      COUNT(*)::BIGINT FROM routes
          UNION ALL
          SELECT 'Total Trips',       COUNT(*)::BIGINT FROM trips
          UNION ALL
          SELECT 'Total Stop Events', COUNT(*)::BIGINT FROM stop_times
        `);

        // Set connection globally for Analytics Studio query lifecycle
        setSharedConnection(conn);
        setLoading(false);
      } catch (err) {
        console.error(err);
        setError(err instanceof Error ? err.message : 'Unknown error');
        setLoading(false);
      }
    }

    initialize();
  }, []);

  if (loading) {
    return (
      <div className="app-loading">
        <div className="app-loading-spinner" aria-label="Initialising DuckDB…">
          <div /><div /><div /><div />
        </div>
        <p className="app-loading-text">Initialising DuckDB WASM...</p>
      </div>
    );
  }

  if (error) {
    return <div className="p-8 text-red-500">Error: {error}</div>;
  }

  return (
    <div className="app-shell">
      <AnalyticsStudio />
    </div>
  );
}

export default App;
