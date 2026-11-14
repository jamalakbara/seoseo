"use client";

import { useEffect, useRef, useState } from "react";
import "maplibre-gl/dist/maplibre-gl.css";

export type MapPoint = { row: number; column: number; latitude: number; longitude: number; rank: number | null; measured: boolean };
export function pointColor(p: Pick<MapPoint, "rank" | "measured">) {
  return !p.measured ? "#64748b" : p.rank === null ? "#b91c1c" : p.rank <= 3 ? "#15803d" : p.rank <= 10 ? "#a16207" : "#c2410c";
}

/** Loaded only on explicit map display. No prefetch or server-side tile proxy. */
export function GridMap({ points, onSelect }: { points: MapPoint[]; onSelect: (row: number, column: number) => void }) {
  const container = useRef<HTMLDivElement>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let disposed = false;
    let cleanup = () => {};
    import("maplibre-gl").then(({ Map, Marker, NavigationControl, setWorkerUrl, getVersion }) => {
      if (disposed || !container.current || !points.length) return;
      setWorkerUrl(`/maplibre/maplibre-gl-worker.mjs?v=${getVersion()}`);
      const tile = process.env.NEXT_PUBLIC_MAP_TILE_URL ?? "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
      const map = new Map({ container: container.current, center: [points[Math.floor(points.length / 2)].longitude, points[Math.floor(points.length / 2)].latitude], zoom: 11,
        style: { version: 8, sources: tile === "none" ? {} : { osm: { type: "raster", tiles: [tile], tileSize: 256, maxzoom: 19,
          attribution: '© <a href="https://www.openstreetmap.org/copyright" target="_blank" rel="noreferrer">OpenStreetMap contributors</a>' } },
        layers: tile === "none" ? [] : [{ id: "osm", type: "raster", source: "osm" }] },
        attributionControl: { compact: false },
      });
      map.on("error", () => { if (!disposed) setError("Some map resources could not load. The numeric grid and point details remain available."); });
      map.addControl(new NavigationControl());
      const markers = points.map(p => {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = !p.measured ? "?" : p.rank === null ? "–" : String(p.rank);
        button.setAttribute("aria-label", `Row ${p.row + 1}, column ${p.column + 1}: ${!p.measured ? "unchecked" : p.rank === null ? "not returned" : `rank ${p.rank}`}`);
        Object.assign(button.style, { borderRadius: "50%", background: pointColor(p), color: "white", border: "2px solid white", width: "30px", height: "30px", cursor: "pointer" });
        button.onclick = () => onSelect(p.row, p.column);
        return new Marker({ element: button }).setLngLat([p.longitude, p.latitude]).addTo(map);
      });
      // Fit the local grid without sweeping other regions or zoom levels.
      const west = Math.min(...points.map(p => p.longitude));
      const east = Math.max(...points.map(p => p.longitude));
      if (east - west < 180) map.fitBounds([[west, Math.min(...points.map(p => p.latitude))], [east, Math.max(...points.map(p => p.latitude))]], { padding: 35, duration: 0, maxZoom: 15 });
      const resize = new ResizeObserver(() => map.resize());
      resize.observe(container.current);
      cleanup = () => { resize.disconnect(); markers.forEach(m => m.remove()); map.remove(); };
    }).catch(() => { if (!disposed) setError("The map could not load. Use the accessible grid below."); });
    return () => { disposed = true; cleanup(); };
  }, [points, onSelect]);
  return <div className="grid gap-2"><div ref={container} className="h-96 w-full rounded border" aria-label="Local ranking map" />
    {error && <p role="alert">{error}</p>}
    <a className="text-xs underline" href="https://www.openstreetmap.org/fixthemap" target="_blank" rel="noreferrer">Report a map issue</a>
  </div>;
}
