<script lang="ts">
  // Full map: Leaflet with CRS.Simple over the original 7800x7817 basemap
  // space, so every px/py from Rust is used directly as a map coordinate.
  // The frontend never runs a world<->pixel transform.
  import { onDestroy, onMount } from "svelte";
  import L from "leaflet";
  import "leaflet/dist/leaflet.css";
  import {
    addWaypointAtPixel,
    deleteWaypoint,
    getBasemapUrls,
    getCurrentTrail,
    getNearestWaypoint,
    getPoisRender,
    getPreviousTrail,
    getSettings,
    listWaypointsPx,
    patchSettings,
    onWaypointsChanged,
    onPositionUpdate,
    onSettingsChanged,
    onTrailChanged,
    renameWaypoint,
    type NearestWaypoint,
    type PoiLayer,
    type PositionUpdate,
    type Settings,
    type TrailPayload,
    type Waypoint,
    type WaypointPx,
  } from "$lib/api";
  import {
    BASEMAP_H,
    BASEMAP_W,
    COLORS,
    LAYER_COLORS,
    LAYER_ORDER,
    PLAYER_DOT_RADIUS,
    POI_DOT_RADIUS,
    WAYPOINT_RADIUS,
    ZONE_FILL_OPACITY,
    ZONE_STROKE_OPACITY,
  } from "$lib/theme";
  import LayerPanel from "./LayerPanel.svelte";
  import NamePrompt from "./NamePrompt.svelte";
  import { tNow } from "$lib/i18n";
  import { ask } from "@tauri-apps/plugin-dialog";

  // Same zoom envelope as the original QGraphicsView (scale 0.04 .. 3.0).
  const MIN_ZOOM = Math.log2(0.04);
  const MAX_ZOOM = Math.log2(3.0);

  const toLatLng = (px: number, py: number): L.LatLngTuple => [-py, px];

  let mapEl: HTMLDivElement;
  let map: L.Map | undefined;
  let layerGroups: Record<string, L.LayerGroup> = {};
  // Zone name labels live in their own groups so the "zone names" toggle can
  // hide the text while the outlines stay.
  let zoneLabelGroups: Record<string, L.LayerGroup> = {};
  let waypointGroup: L.LayerGroup | undefined;
  let currentTrail: L.LayerGroup | undefined;
  let previousTrail: L.LayerGroup | undefined;
  let playerMarker: L.CircleMarker | undefined;

  let settings = $state<Settings | null>(null);
  let position = $state<PositionUpdate | null>(null);
  let nearest = $state<NearestWaypoint | null>(null);
  let availableLayers = $state<string[]>([]);
  let promptOpen = $state(false);
  let pendingPixel: { px: number; py: number } | null = null;

  const unlisteners: Array<() => void> = [];

  const escapeHtml = (s: string) =>
    s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

  function buildPoiLayers(pois: PoiLayer[]) {
    if (!map) return;
    const byKey = new Map(pois.map((l) => [l.key, l]));
    for (const key of LAYER_ORDER) {
      const layer = byKey.get(key);
      if (!layer) continue;
      const color = LAYER_COLORS[key] ?? COLORS.accent;
      const group = L.layerGroup();
      const labelGroup = layer.kind === "zone" ? L.layerGroup() : undefined;
      for (const item of layer.items) {
        if (layer.kind === "label") {
          // Pure text label (region/landmark names) — no shape.
          L.marker(toLatLng(item.px, item.py), {
            icon: L.divIcon({
              className: `map-label map-label--${key}`,
              html: escapeHtml(item.label),
              iconSize: undefined,
            }),
            interactive: false,
            keyboard: false,
          }).addTo(group);
          continue;
        }
        if (
          labelGroup &&
          item.label &&
          item.labelPx !== undefined &&
          item.labelPy !== undefined
        ) {
          // Permanent name at the zone's centre, colour-matched to its layer.
          L.tooltip({
            permanent: true,
            direction: "center",
            className: "zone-label",
            opacity: 1,
            interactive: false,
          })
            .setContent(
              `<span style="color: ${color}">${escapeHtml(item.label)}</span>`,
            )
            .setLatLng(toLatLng(item.labelPx, item.labelPy))
            .addTo(labelGroup);
        }
        if (item.pointsPx) {
          L.polygon(item.pointsPx.map(([px, py]) => toLatLng(px, py)), {
            color,
            weight: 1.6,
            opacity: ZONE_STROKE_OPACITY,
            fillColor: color,
            fillOpacity: ZONE_FILL_OPACITY,
          })
            .bindTooltip(item.label, { sticky: true })
            .addTo(group);
        } else if (item.radiusPx) {
          // CRS.Simple: L.circle radius is in map units = basemap pixels.
          L.circle(toLatLng(item.px, item.py), {
            radius: item.radiusPx,
            color,
            weight: 1.6,
            opacity: ZONE_STROKE_OPACITY,
            fillColor: color,
            fillOpacity: ZONE_FILL_OPACITY,
          })
            .bindTooltip(item.label, { sticky: true })
            .addTo(group);
        } else {
          // Fixed screen-size dot at any zoom (circleMarker radius is px).
          L.circleMarker(toLatLng(item.px, item.py), {
            radius: POI_DOT_RADIUS,
            color: "rgba(0,0,0,0.63)",
            weight: 1,
            fillColor: color,
            fillOpacity: 1,
          })
            .bindTooltip(item.label)
            .addTo(group);
        }
      }
      layerGroups[key] = group;
      if (settings?.layers?.[key] ?? true) group.addTo(map);
      if (labelGroup) {
        zoneLabelGroups[key] = labelGroup;
        if ((settings?.layers?.[key] ?? true) && (settings?.map?.zone_labels ?? true)) {
          labelGroup.addTo(map);
        }
      }
    }
    availableLayers = LAYER_ORDER.filter((k) => byKey.has(k));
  }

  function drawTrail(target: L.LayerGroup, trail: TrailPayload, dimmed: boolean) {
    target.clearLayers();
    for (const seg of trail.segmentsPx) {
      if (seg.length < 2) continue;
      L.polyline(seg.map(([px, py]) => toLatLng(px, py)), {
        color: COLORS.trail,
        weight: 2,
        opacity: dimmed ? 0.35 : 0.9,
        dashArray: dimmed ? "6 6" : undefined,
        interactive: false,
      }).addTo(target);
    }
  }

  let waypointsPx = $state<WaypointPx[]>([]);

  async function refreshWaypoints() {
    // px/py for rendering come from Rust — the transform stays single-sourced.
    waypointsPx = await listWaypointsPx();
    if (!map || !waypointGroup) return;
    waypointGroup.clearLayers();
    for (const wp of waypointsPx) {
      L.circleMarker(toLatLng(wp.px, wp.py), {
        radius: WAYPOINT_RADIUS,
        color: "rgba(0,0,0,0.78)",
        weight: 1.2,
        fillColor: wp.color ?? COLORS.waypoint,
        fillOpacity: 1,
      })
        .bindTooltip(wp.name)
        .addTo(waypointGroup);
    }
    nearest = await getNearestWaypoint();
  }

  function applyLayerVisibility(layers: Record<string, boolean>, zoneLabels: boolean) {
    if (!map) return;
    const setVisible = (group: L.LayerGroup, visible: boolean) => {
      if (visible && !map!.hasLayer(group)) group.addTo(map!);
      if (!visible && map!.hasLayer(group)) map!.removeLayer(group);
    };
    for (const [key, group] of Object.entries(layerGroups)) {
      setVisible(group, layers[key] ?? true);
    }
    for (const [key, group] of Object.entries(zoneLabelGroups)) {
      setVisible(group, (layers[key] ?? true) && zoneLabels);
    }
  }

  const zoneLabelsOn = (s: Settings | null) => s?.map?.zone_labels ?? true;

  async function onToggleLayer(key: string, visible: boolean) {
    // Persisted (bug fix 1) — settings://changed loops back to every window,
    // including the minimap's POI filter.
    settings = await patchSettings({ layers: { [key]: visible } });
    applyLayerVisibility(settings.layers, zoneLabelsOn(settings));
  }

  async function onToggleZoneLabels(visible: boolean) {
    settings = await patchSettings({ map: { zone_labels: visible } });
    applyLayerVisibility(settings.layers, zoneLabelsOn(settings));
  }

  async function confirmPrompt(name: string) {
    promptOpen = false;
    if (!pendingPixel) return;
    await addWaypointAtPixel(pendingPixel.px, pendingPixel.py, name || tNow("wp.new"));
    pendingPixel = null;
    await refreshWaypoints();
  }

  async function onRename(id: string, name: string) {
    await renameWaypoint(id, name);
    await refreshWaypoints();
  }

  async function onDelete(wp: Waypoint) {
    const yes = await ask(tNow("wp.confirm_delete", { name: wp.name }), {
      title: tNow("wp.title"),
      kind: "warning",
    });
    if (!yes) return;
    await deleteWaypoint(wp.id);
    await refreshWaypoints();
  }

  function focusWaypoint(wp: Waypoint) {
    const found = waypointsPx.find((w) => w.id === wp.id);
    if (map && found) map.panTo(toLatLng(found.px, found.py));
  }

  onMount(() => {
    (async () => {
      settings = await getSettings();

      map = L.map(mapEl, {
        crs: L.CRS.Simple,
        minZoom: MIN_ZOOM,
        maxZoom: MAX_ZOOM,
        zoomSnap: 0,
        zoomDelta: 0.25,
        wheelPxPerZoomLevel: 90,
        attributionControl: false,
        zoomControl: true,
      });
      const bounds: L.LatLngBoundsExpression = [
        [-BASEMAP_H, 0],
        [0, BASEMAP_W],
      ];
      const urls = await getBasemapUrls();
      L.imageOverlay(urls.fullmap, bounds).addTo(map);
      map.fitBounds(bounds);
      map.setMaxBounds([
        [-BASEMAP_H * 1.15, -BASEMAP_W * 0.15],
        [BASEMAP_H * 0.15, BASEMAP_W * 1.15],
      ]);

      previousTrail = L.layerGroup().addTo(map);
      currentTrail = L.layerGroup().addTo(map);
      waypointGroup = L.layerGroup().addTo(map);

      try {
        buildPoiLayers(await getPoisRender());
      } catch {
        // POI data missing (partial first run): map works without dots.
      }
      drawTrail(previousTrail, await getPreviousTrail(), true);
      drawTrail(currentTrail, await getCurrentTrail(), false);
      await refreshWaypoints();

      map.on("contextmenu", (e: L.LeafletMouseEvent) => {
        pendingPixel = { px: e.latlng.lng, py: -e.latlng.lat };
        promptOpen = true;
      });

      unlisteners.push(
        await onPositionUpdate(async (p) => {
          position = p;
          if (!map) return;
          const ll = toLatLng(p.px, p.py);
          if (!playerMarker) {
            playerMarker = L.circleMarker(ll, {
              radius: PLAYER_DOT_RADIUS,
              color: "rgba(255,255,255,0.86)",
              weight: 1.5,
              fillColor: COLORS.player,
              fillOpacity: 1,
              interactive: false,
            }).addTo(map);
          } else {
            playerMarker.setLatLng(ll);
          }
          map.panTo(ll);
          nearest = await getNearestWaypoint();
        }),
      );
      unlisteners.push(
        await onTrailChanged((trail) => {
          if (currentTrail) drawTrail(currentTrail, trail, false);
        }),
      );
      unlisteners.push(
        await onSettingsChanged((s) => {
          settings = s;
          applyLayerVisibility(s.layers, zoneLabelsOn(s));
        }),
      );
      // Hotkey "mark here" adds waypoints from Rust — refresh on its signal.
      unlisteners.push(await onWaypointsChanged(() => void refreshWaypoints()));
    })();

    return () => unlisteners.forEach((u) => u());
  });

  onDestroy(() => {
    map?.remove();
    map = undefined;
  });
</script>

<div class="flex h-full min-h-0">
  <div class="min-w-0 flex-1" bind:this={mapEl} style="background: var(--color-bg)"></div>
  <LayerPanel
    available={availableLayers}
    layers={settings?.layers ?? {}}
    zoneLabels={zoneLabelsOn(settings)}
    {position}
    {nearest}
    waypoints={waypointsPx}
    ontoggle={onToggleLayer}
    ontogglezonelabels={onToggleZoneLabels}
    onrename={onRename}
    ondelete={onDelete}
    onfocus={focusWaypoint}
  />
</div>

<NamePrompt
  open={promptOpen}
  title={tNow("wp.new")}
  label={tNow("wp.name_prompt")}
  onconfirm={confirmPrompt}
  oncancel={() => {
    promptOpen = false;
    pendingPixel = null;
  }}
/>

<style>
  :global(.leaflet-container) {
    background: var(--color-bg);
    font-family: "Segoe UI", system-ui, sans-serif;
  }
  :global(.leaflet-tooltip) {
    background: var(--color-panel);
    color: var(--color-text);
    border: 1px solid var(--color-border);
  }
  :global(.leaflet-tooltip-top:before),
  :global(.leaflet-tooltip-bottom:before),
  :global(.leaflet-tooltip-left:before),
  :global(.leaflet-tooltip-right:before) {
    border-top-color: var(--color-border);
  }
  :global(.leaflet-bar a) {
    background: var(--color-panel);
    color: var(--color-text);
    border-bottom: 1px solid var(--color-border);
  }
  :global(.leaflet-bar a:hover) {
    background: var(--color-bg);
  }

  /* Text-label layers (region/landmark names). The dark 1px shadow makes
     text readable over bright terrain without any outline box — same trick
     as the minimap compass letters. */
  :global(.map-label) {
    width: max-content !important;
    height: auto !important;
    margin: 0 !important;
    transform: translate(-50%, -50%);
    white-space: nowrap;
    pointer-events: none;
    text-shadow:
      1px 1px 2px rgba(0, 0, 0, 0.9),
      -1px -1px 2px rgba(0, 0, 0, 0.7);
    font-family: "Segoe UI", system-ui, sans-serif;
  }
  :global(.map-label--region) {
    color: #eae6d6;
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    opacity: 0.85;
  }
  :global(.map-label--landmark) {
    color: #cfc9b3;
    font-size: 11.5px;
    font-weight: 500;
  }
  :global(.map-label--landmark)::before {
    content: "";
    display: inline-block;
    width: 5px;
    height: 5px;
    margin-right: 4px;
    margin-bottom: 1px;
    border-radius: 50%;
    background: #cfc9b3;
    box-shadow: 0 0 2px rgba(0, 0, 0, 0.9);
  }

  /* Zone name labels: plain colour-matched text, no tooltip bubble. */
  :global(.leaflet-tooltip.zone-label) {
    background: transparent;
    border: none;
    box-shadow: none;
    font-size: 11.5px;
    font-weight: 600;
    text-shadow:
      1px 1px 2px rgba(0, 0, 0, 0.9),
      -1px -1px 2px rgba(0, 0, 0, 0.7);
    pointer-events: none;
  }
  :global(.leaflet-tooltip.zone-label)::before {
    display: none;
  }
</style>
