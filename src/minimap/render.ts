// Circular minimap renderer — full port of MinimapWindow.paintEvent.
//
// The map NEVER rotates: north is always up, so the compass letters stay
// put. The player's heading is shown by the arrow and the readout pill.
// Drawn with one drawImage cropping the region around the player out of the
// preloaded 975 px tier. No repaint timers: draw only on new data.

export interface PoiDot {
  xCm: number;
  yCm: number;
  px: number; // basemap 7800-space pixels
  py: number;
  color: string;
}

export interface DinoBars {
  hp: { current: number | null; max: number | null };
  hunger: { current: number | null; max: number | null };
  thirst: { current: number | null; max: number | null };
  growthPct: number | null;
}

/** Must match DINO_PANEL_H in src-tauri/src/minimap.rs. */
export const PANEL_H = 76;

export interface MinimapState {
  /** Player position (cm + basemap px) and heading, or null before first sample. */
  position: { xCm: number; yCm: number; px: number; py: number; headingDeg: number | null } | null;
  /** Trail segments in basemap px. */
  trailPx: [number, number][][];
  /** Point POIs already filtered by layer visibility (not by distance). */
  pois: PoiDot[];
  basemap: ImageBitmap | null;
  /** basemap tier scale: tierWidth / 7800. */
  miniScale: number;
  /** Basemap px per real metre (horizontal). */
  pxPerM: number;
  sizePx: number;
  radiusM: number;
  opacity: number;
  /** Extra height for the dino-stats strip; 0 = strip off. */
  panelH: number;
  dino: DinoBars | null;
  /** Localised strings: compass letters clockwise from north, hint, unknown. */
  compassLetters: [string, string, string, string];
  hintText: string;
  headingLabel: string; // "" when unknown -> shows headingUnknown
  headingUnknown: string;
}

const LABEL_MARGIN = 15;
const POI_MARGIN = 1.6; // filter wider than the view so dots don't pop in at the rim

const COLORS = {
  bg: "#11150e",
  text: "#eae6d6",
  textMuted: "#a3aa8c",
  accent: "#e8a33d",
  // Electric yellow + double outline (dark under, white over): the
  // self-marker must never be confused with POI dots or the softer trail.
  playerArrow: "#ffe600",
  playerArrowOutline: "#10130c",
  playerHalo: "rgba(255, 230, 0, 0.20)",
  trail: "#ffcc55",
};

export function render(canvas: HTMLCanvasElement, state: MinimapState): void {
  const size = state.sizePx;
  const totalH = size + state.panelH;
  const dpr = window.devicePixelRatio || 1;
  if (
    canvas.width !== Math.round(size * dpr) ||
    canvas.height !== Math.round(totalH * dpr)
  ) {
    canvas.width = Math.round(size * dpr);
    canvas.height = Math.round(totalH * dpr);
    canvas.style.width = `${size}px`;
    canvas.style.height = `${totalH}px`;
  }
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, size, totalH);

  if (state.panelH > 0) {
    drawDinoPanel(ctx, state, size);
  }

  const c = size / 2;
  const radius = size / 2 - LABEL_MARGIN;

  if (!state.position) {
    // No position yet: a dim disc so the hint text is readable.
    ctx.beginPath();
    ctx.arc(c, c, radius, 0, Math.PI * 2);
    ctx.fillStyle = "rgba(17, 21, 14, 0.88)";
    ctx.fill();
    drawHint(ctx, c, radius, state.hintText);
    return;
  }

  ctx.save();
  ctx.beginPath();
  ctx.arc(c, c, radius, 0, Math.PI * 2);
  ctx.clip();
  drawMap(ctx, state, c, radius);
  ctx.restore();

  drawCompass(ctx, state, c, radius);
  drawHeadingPill(ctx, state, c, radius);
  // Player marker LAST and always fully opaque: however faded the map is,
  // you must still see where you are or the whole map is pointless.
  drawPlayer(ctx, state, c);
}

function drawMap(
  ctx: CanvasRenderingContext2D,
  state: MinimapState,
  c: number,
  radius: number,
): void {
  const pos = state.position!;
  const sceneR = state.radiusM * state.pxPerM; // view radius in basemap px
  const side = radius * 2;
  const ox = c - radius;
  const oy = c - radius;

  if (state.basemap) {
    const s = state.miniScale;
    ctx.globalAlpha = state.opacity;
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(
      state.basemap,
      (pos.px - sceneR) * s,
      (pos.py - sceneR) * s,
      sceneR * 2 * s,
      sceneR * 2 * s,
      ox,
      oy,
      side,
      side,
    );
    ctx.globalAlpha = 1;
  }

  const toWidget = (sx: number, sy: number): [number, number] => [
    ox + ((sx - (pos.px - sceneR)) / (sceneR * 2)) * side,
    oy + ((sy - (pos.py - sceneR)) / (sceneR * 2)) * side,
  ];

  // Trail.
  ctx.strokeStyle = COLORS.trail;
  ctx.lineWidth = 2;
  ctx.lineJoin = "round";
  for (const seg of state.trailPx) {
    if (seg.length < 2) continue;
    ctx.beginPath();
    const [x0, y0] = toWidget(seg[0][0], seg[0][1]);
    ctx.moveTo(x0, y0);
    for (let i = 1; i < seg.length; i++) {
      const [x, y] = toWidget(seg[i][0], seg[i][1]);
      ctx.lineTo(x, y);
    }
    ctx.stroke();
  }

  // POI dots, distance-filtered (in metres, straight from cm).
  const limitM = state.radiusM * POI_MARGIN;
  ctx.strokeStyle = "rgba(0, 0, 0, 0.59)";
  ctx.lineWidth = 1;
  for (const poi of state.pois) {
    const distM = Math.hypot(poi.xCm - pos.xCm, poi.yCm - pos.yCm) / 100;
    if (distM > limitM) continue;
    const [x, y] = toWidget(poi.px, poi.py);
    ctx.beginPath();
    ctx.arc(x, y, 3.5, 0, Math.PI * 2);
    ctx.fillStyle = poi.color;
    ctx.fill();
    ctx.stroke();
  }
}

function drawCompass(
  ctx: CanvasRenderingContext2D,
  state: MinimapState,
  c: number,
  radius: number,
): void {
  // Four letters around the disc. No ring, no ticks: each letter gets a
  // 1 px offset shadow instead — enough to separate it from bright terrain
  // without drawing any outline.
  ctx.font = "bold 13px 'Segoe UI', sans-serif";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  const labelR = radius + LABEL_MARGIN / 2 + 2;
  ctx.globalAlpha = state.opacity;
  const angles = [0, 90, 180, 270];
  for (let i = 0; i < 4; i++) {
    const rad = ((angles[i] - 90) * Math.PI) / 180;
    const x = c + labelR * Math.cos(rad);
    const y = c + labelR * Math.sin(rad);
    ctx.fillStyle = "rgba(0, 0, 0, 0.75)";
    ctx.fillText(state.compassLetters[i], x + 1, y + 1);
    // North in the accent colour so a glance finds it.
    ctx.fillStyle = angles[i] === 0 ? COLORS.accent : COLORS.text;
    ctx.fillText(state.compassLetters[i], x, y);
  }
  ctx.globalAlpha = 1;
}

function drawPlayer(ctx: CanvasRenderingContext2D, state: MinimapState, c: number): void {
  const heading = state.position!.headingDeg;

  ctx.beginPath();
  ctx.arc(c, c, 13, 0, Math.PI * 2);
  ctx.fillStyle = COLORS.playerHalo;
  ctx.fill();

  if (heading !== null) {
    // Compass bearing 0 = north = up; canvas rotate() is clockwise in
    // y-down coordinates, so the bearing maps 1:1. Dart shape (tip ahead,
    // notched tail) centred on the player.
    ctx.save();
    ctx.translate(c, c);
    ctx.rotate((heading * Math.PI) / 180);
    ctx.beginPath();
    ctx.moveTo(0, -14);
    ctx.lineTo(9, 11);
    ctx.lineTo(0, 5);
    ctx.lineTo(-9, 11);
    ctx.closePath();
    ctx.lineJoin = "round";
    ctx.strokeStyle = COLORS.playerArrowOutline;
    ctx.lineWidth = 3.5;
    ctx.stroke();
    ctx.fillStyle = COLORS.playerArrow;
    ctx.fill();
    ctx.strokeStyle = "rgba(255, 255, 255, 0.9)";
    ctx.lineWidth = 1.2;
    ctx.stroke();
    ctx.restore();
  } else {
    // Heading unknown: a plain disc implies no direction (the pill below
    // says why); same yellow + double outline keeps it unmistakably "you".
    ctx.beginPath();
    ctx.arc(c, c, 7, 0, Math.PI * 2);
    ctx.strokeStyle = COLORS.playerArrowOutline;
    ctx.lineWidth = 3.5;
    ctx.stroke();
    ctx.fillStyle = COLORS.playerArrow;
    ctx.fill();
    ctx.strokeStyle = "rgba(255, 255, 255, 0.9)";
    ctx.lineWidth = 1.5;
    ctx.stroke();
  }
}

function drawHeadingPill(
  ctx: CanvasRenderingContext2D,
  state: MinimapState,
  c: number,
  radius: number,
): void {
  const known = state.headingLabel !== "";
  const text = known ? state.headingLabel : state.headingUnknown;
  ctx.font = "600 12px 'Segoe UI', sans-serif";
  const w = ctx.measureText(text).width + 16;
  const h = 20;
  const x = c - w / 2;
  const y = c + radius * 0.52;

  ctx.globalAlpha = state.opacity;
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, h / 2);
  ctx.fillStyle = "rgba(0, 0, 0, 0.67)";
  ctx.fill();
  ctx.fillStyle = known ? COLORS.accent : COLORS.textMuted;
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText(text, c, y + h / 2 + 0.5);
  ctx.globalAlpha = 1;
}

/// Compact "your dino" strip below the disc: HP / hunger / thirst bars plus
/// growth. Drawn with the same opacity as the map so the whole widget reads
/// as one block; text keeps a dark shadow for readability.
function drawDinoPanel(ctx: CanvasRenderingContext2D, state: MinimapState, size: number): void {
  const top = size + 2;
  const h = state.panelH - 4;
  ctx.save();
  ctx.globalAlpha = Math.max(state.opacity, 0.55);

  // Backing card.
  ctx.beginPath();
  ctx.roundRect(4, top, size - 8, h, 8);
  ctx.fillStyle = "rgba(10, 13, 9, 0.78)";
  ctx.fill();

  const dino = state.dino;
  const rows: Array<{ label: string; cur: number | null; max: number | null; color: string }> =
    dino
      ? [
          {
            label: "HP",
            cur: dino.hp.current,
            max: dino.hp.max,
            color:
              dino.hp.current !== null && dino.hp.max
                ? dino.hp.current / dino.hp.max > 0.5
                  ? "#72d653"
                  : dino.hp.current / dino.hp.max > 0.25
                    ? "#e8a33d"
                    : "#e2664a"
                : "#72d653",
          },
          { label: "\u{1F356}", cur: dino.hunger.current, max: dino.hunger.max, color: "#e8a33d" },
          { label: "\u{1F4A7}", cur: dino.thirst.current, max: dino.thirst.max, color: "#4aa8d8" },
        ]
      : [];

  ctx.font = "600 10px 'Segoe UI', sans-serif";
  ctx.textBaseline = "middle";

  if (!dino) {
    ctx.fillStyle = COLORS.textMuted;
    ctx.textAlign = "center";
    ctx.fillText("…", size / 2, top + h / 2);
    ctx.restore();
    return;
  }

  const rowH = 16;
  const barX = 30;
  const barW = size - 8 - barX - 44;
  rows.forEach((row, i) => {
    const y = top + 6 + i * rowH + rowH / 2;
    ctx.textAlign = "left";
    ctx.fillStyle = COLORS.text;
    ctx.fillText(row.label, 10, y);

    ctx.beginPath();
    ctx.roundRect(barX, y - 3.5, barW, 7, 3.5);
    ctx.fillStyle = "rgba(255,255,255,0.12)";
    ctx.fill();
    if (row.cur !== null && row.max) {
      const frac = Math.max(0, Math.min(1, row.cur / row.max));
      if (frac > 0) {
        ctx.beginPath();
        ctx.roundRect(barX, y - 3.5, Math.max(barW * frac, 3), 7, 3.5);
        ctx.fillStyle = row.color;
        ctx.fill();
      }
    }

    ctx.textAlign = "right";
    ctx.fillStyle = COLORS.text;
    ctx.fillText(
      row.cur !== null && row.max !== null ? `${Math.round(row.cur)}/${Math.round(row.max)}` : "—",
      size - 12,
      y,
    );
  });

  // Growth line.
  const gy = top + 6 + rows.length * rowH + 6;
  ctx.textAlign = "left";
  ctx.fillStyle = COLORS.accent;
  ctx.fillText(
    dino.growthPct !== null ? `Growth ${Math.round(dino.growthPct)}%` : "Growth —",
    10,
    gy,
  );
  ctx.restore();
}

function drawHint(
  ctx: CanvasRenderingContext2D,
  c: number,
  radius: number,
  hint: string,
): void {
  ctx.fillStyle = COLORS.textMuted;
  ctx.font = "12px 'Segoe UI', sans-serif";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  // Simple greedy word wrap inside the disc.
  const maxWidth = radius * 2 - 44;
  const words = hint.split(" ");
  const lines: string[] = [];
  let line = "";
  for (const word of words) {
    const probe = line ? `${line} ${word}` : word;
    if (ctx.measureText(probe).width > maxWidth && line) {
      lines.push(line);
      line = word;
    } else {
      line = probe;
    }
  }
  if (line) lines.push(line);
  const lineH = 16;
  const y0 = c - ((lines.length - 1) * lineH) / 2;
  lines.forEach((l, i) => ctx.fillText(l, c, y0 + i * lineH));
}
