import React, { useEffect, useState } from 'react';
import { Vehicle } from '@transit-intelligence/shared-types';

export default function App() {
  const [vehicles, setVehicles] = useState<Vehicle[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetch('http://localhost:3000/api/v1/vehicles/active')
      .then((res) => {
        if (!res.ok) throw new Error('Failed to fetch active vehicles');
        return res.json();
      })
      .then((data) => {
        setVehicles(data);
        setLoading(false);
      })
      .catch((err) => {
        setError(err.message);
        setLoading(false);
      });
  }, []);

  return (
    <div className="dashboard-container">
      <header className="dashboard-header">
        <div className="header-brand">
          <span className="brand-logo">🚌</span>
          <h1>Transit Intelligence</h1>
        </div>
        <div className="system-badge">System Status: Active</div>
      </header>

      <main className="dashboard-grid">
        <section className="metrics-section">
          <div className="card metric-card">
            <h3>Active Vehicles</h3>
            <p className="metric-value">{vehicles.length}</p>
            <span className="metric-subtext">Real-time tracked fleets</span>
          </div>
          <div className="card metric-card">
            <h3>Network Ingest</h3>
            <p className="metric-value">0 ms</p>
            <span className="metric-subtext">Redis Stream latency</span>
          </div>
          <div className="card metric-card">
            <h3>Data Pipeline</h3>
            <p className="metric-value">Phase 1</p>
            <span className="metric-subtext">Postgres + DuckDB</span>
          </div>
        </section>

        <section className="fleet-section card">
          <div className="section-header">
            <h2>Live Fleet Status</h2>
            <button className="btn btn-refresh" onClick={() => window.location.reload()}>
              Refresh
            </button>
          </div>

          {loading && <div className="loading-state">Loading active vehicles...</div>}
          {error && (
            <div className="error-state">
              Connection status: Local API disconnected (Placeholder active)
            </div>
          )}

          {!loading && (
            <div className="table-responsive">
              <table className="data-table">
                <thead>
                  <tr>
                    <th>Vehicle ID</th>
                    <th>License Plate</th>
                    <th>Status</th>
                    <th>Capacity</th>
                    <th>Agency</th>
                  </tr>
                </thead>
                <tbody>
                  {vehicles.length === 0 ? (
                    <tr>
                      <td colSpan={5} className="no-data">
                        No active vehicles found
                      </td>
                    </tr>
                  ) : (
                    vehicles.map((v) => (
                      <tr key={v.id}>
                        <td>
                          <code>{v.id}</code>
                        </td>
                        <td>{v.licensePlate}</td>
                        <td>
                          <span className={`badge badge-${v.status.toLowerCase()}`}>
                            {v.status}
                          </span>
                        </td>
                        <td>{v.capacity} pax</td>
                        <td>{v.agencyId}</td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          )}
        </section>
      </main>
    </div>
  );
}
