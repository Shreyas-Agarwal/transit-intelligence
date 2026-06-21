import { useEffect, useRef, useMemo } from 'react';
import maplibregl from 'maplibre-gl';
import 'maplibre-gl/dist/maplibre-gl.css';

import type { Widget } from '../../../analytics/registry/types';
import { useDashboardStore } from '../../../store/dashboardStore';
import { useBenchmarkStore } from '../../../store/benchmarkStore';
import WidgetShell from './WidgetShell';

//-------------------------------------
// Map Widget
//
// Renders geospatial points on MapLibre GL from query coordinates.
// Responsibilities:
//   - Render stop points based on query lat/lon
//   - Filter or highlight selected points using WebGL painting properties
//   - Click point -> set highlight context
//   - Handle card resize using local ResizeObserver
//-------------------------------------

interface Props {
  widget: Widget;
}

export default function MapWidget({ widget }: Props) {
  const mapContainerRef = useRef<HTMLDivElement>(null);
  const mapRef = useRef<maplibregl.Map | null>(null);

  const highlightContext = useDashboardStore((s) => s.highlightContext);
  const theme = useDashboardStore((s) => s.theme);
  const setHighlight = useDashboardStore((s) => s.setHighlight);
  const clearHighlight = useDashboardStore((s) => s.clearHighlight);

  const rows = useMemo(() => widget.result?.rows ?? [], [widget.result?.rows]);

  // Adjust tile shading filter dynamically when theme changes
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;

    const updateShading = () => {
      const layerId = 'osm-layer';
      if (!map.getLayer(layerId)) return;

      map.setPaintProperty(layerId, 'raster-brightness-max', theme === 'light' ? 0.95 : 0.6);
      map.setPaintProperty(layerId, 'raster-contrast', theme === 'light' ? 0.1 : 0.15);
      map.setPaintProperty(layerId, 'raster-saturation', theme === 'light' ? -0.4 : -0.85);
    };

    if (map.isStyleLoaded()) {
      updateShading();
    } else {
      map.once('load', updateShading);
    }
  }, [theme]);

  // 1. Initialize MapLibre GL instance
  useEffect(() => {
    if (!mapContainerRef.current) return;

    const renderStart = performance.now();

    // Dark-themed desaturated OSM style
    const map = new maplibregl.Map({
      container: mapContainerRef.current,
      style: {
        version: 8,
        sources: {
          osm: {
            type: 'raster',
            tiles: ['https://a.tile.openstreetmap.org/{z}/{x}/{y}.png'],
            tileSize: 256,
            attribution: '© OpenStreetMap',
          },
        },
        layers: [
          {
            id: 'osm-layer',
            type: 'raster',
            source: 'osm',
            paint: {
              'raster-brightness-max': 0.6,
              'raster-contrast': 0.15,
              'raster-saturation': -0.85,
            },
          },
        ],
      },
      center: [8.54, 47.37], // Zurich center
      zoom: 11.5,
    });

    mapRef.current = map;

    map.on('load', () => {
      const ms = performance.now() - renderStart;
      useBenchmarkStore.getState().addEvent({
        eventType: 'map-render',
        durationMs: ms,
        widgetId: widget.id,
      });
    });

    return () => {
      map.remove();
      mapRef.current = null;
    };
  }, [widget.id]);

  // 2. Load stops coordinates onto map as GeoJSON circles
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;

    const loadData = () => {
      const sourceId = 'stops-source';
      const layerId = 'stops-layer';

      const features = rows
        .filter((r) => r.lat != null && r.lon != null)
        .map((r) => ({
          type: 'Feature' as const,
          geometry: {
            type: 'Point' as const,
            coordinates: [Number(r.lon), Number(r.lat)],
          },
          properties: {
            name: String(r.name ?? ''),
            value: r.value != null ? Number(r.value) : null,
          },
        }));

      const geojson = {
        type: 'FeatureCollection' as const,
        features,
      };

      const existingSource = map.getSource(sourceId) as maplibregl.GeoJSONSource | undefined;

      const baseSize = widget.mapPinSize || 6;
      const radiusExpr = (widget.measureCol 
        ? [
            'interpolate',
            ['linear'],
            ['coalesce', ['get', 'value'], baseSize],
            0, baseSize * 0.6,
            50, baseSize * 1.0,
            500, baseSize * 1.8,
            5000, baseSize * 3.0
          ]
        : baseSize) as maplibregl.DataDrivenPropertyValueSpecification<number>;

      if (existingSource) {
        existingSource.setData(geojson);
        if (map.getLayer(layerId)) {
          map.setPaintProperty(layerId, 'circle-radius', radiusExpr);
        }
      } else {
        map.addSource(sourceId, {
          type: 'geojson',
          data: geojson,
        });

        map.addLayer({
          id: layerId,
          type: 'circle',
          source: sourceId,
          paint: {
            'circle-radius': radiusExpr,
            'circle-color': '#818cf8',
            'circle-stroke-color': '#0f1019',
            'circle-stroke-width': 1.5,
            'circle-opacity': 0.85,
          },
        });

        // Click handler to trigger cross-highlighting
        map.on('click', layerId, (e) => {
          const feature = e.features?.[0];
          if (feature) {
            const clickedName = feature.properties?.name;
            if (clickedName && widget.dimensionCol) {
              if (highlightContext.column === widget.dimensionCol && highlightContext.value === clickedName) {
                clearHighlight();
              } else {
                setHighlight({
                  column: widget.dimensionCol,
                  value: clickedName,
                });
              }
            }
          }
        });

        map.on('mouseenter', layerId, () => {
          map.getCanvas().style.cursor = 'pointer';
        });
        map.on('mouseleave', layerId, () => {
          map.getCanvas().style.cursor = '';
        });
      }
    };

    if (map.isStyleLoaded()) {
      loadData();
    } else {
      map.once('load', loadData);
    }
  }, [rows, widget.dimensionCol, widget.mapPinSize, widget.measureCol, highlightContext, setHighlight, clearHighlight]);

  // 3. Highlight/dim layers based on dashboard highlight context
  useEffect(() => {
    const map = mapRef.current;
    if (!map) return;

    const updateHighlight = () => {
      const layerId = 'stops-layer';
      if (!map.getLayer(layerId)) return;

      if (highlightContext.value && highlightContext.column === widget.dimensionCol) {
        map.setPaintProperty(layerId, 'circle-opacity', [
          'case',
          ['==', ['get', 'name'], highlightContext.value],
          0.95,
          0.15,
        ]);
        map.setPaintProperty(layerId, 'circle-color', [
          'case',
          ['==', ['get', 'name'], highlightContext.value],
          '#f472b6', // selection color: pink-400
          '#818cf8',
        ]);
      } else {
        map.setPaintProperty(layerId, 'circle-opacity', 0.85);
        map.setPaintProperty(layerId, 'circle-color', '#818cf8');
      }

      if (highlightContext.appliedAt != null) {
        const ms = performance.now() - highlightContext.appliedAt;
        useBenchmarkStore.getState().addEvent({
          eventType: 'cross-highlight',
          durationMs: ms,
          widgetId: widget.id,
          metadata: { column: highlightContext.column, value: highlightContext.value }
        });
      }
    };

    if (map.isStyleLoaded()) {
      updateHighlight();
    } else {
      map.once('load', updateHighlight);
    }
  }, [highlightContext, widget.dimensionCol, widget.id]);

  // Trigger map resize when container element is resized
  useEffect(() => {
    const map = mapRef.current;
    if (!map || !mapContainerRef.current) return;

    const observer = new ResizeObserver(() => {
      map.resize();
    });
    observer.observe(mapContainerRef.current);
    return () => observer.disconnect();
  }, [rows]);

  return (
    <WidgetShell widget={widget}>
      <div className="map-widget-body" style={{ height: '100%', minHeight: '100%', width: '100%', position: 'relative' }}>
        <div ref={mapContainerRef} style={{ position: 'absolute', top: 0, bottom: 0, left: 0, right: 0, height: '100%', width: '100%' }} />
        {rows.length === 0 && !widget.isLoading && (
          <p className="widget-no-data" style={{ position: 'absolute', top: '50%', left: '50%', transform: 'translate(-50%, -50%)', pointerEvents: 'none' }}>
            No stop coordinates returned. Ensure stops schema matches.
          </p>
        )}
      </div>
    </WidgetShell>
  );
}
