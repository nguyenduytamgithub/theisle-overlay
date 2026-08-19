// English translations. Typed as Record<MsgKey, string>: a missing key is a
// COMPILE error, so the two languages cannot drift apart.

import type { MsgKey } from "./vi";

export const en: Record<MsgKey, string> = {
  "app.title": "The Isle Map",
  "app.minimap_title": "Minimap",
  "app.fullmap_title": "Gateway Map",

  "tab.map": "Map",
  "tab.settings": "Settings",
  "tab.guide": "Guide",

  "pos.none": "No position yet",
  "pos.hint":
    "In game press Tab, then click “Asset Location” in the top-right corner to copy your coordinates.",
  "pos.off_map": "Off the map",

  "dir.N": "North",
  "dir.NE": "North-East",
  "dir.E": "East",
  "dir.SE": "South-East",
  "dir.S": "South",
  "dir.SW": "South-West",
  "dir.W": "West",
  "dir.NW": "North-West",
  "heading.unknown": "Heading unknown",
  "heading.hint": "Copy your coordinates again after moving to reveal your heading.",

  "layer.water": "Water",
  "layer.sanctuary": "Sanctuaries",
  "layer.migration": "Migration zones",
  "layer.saltlick": "Salt licks",
  "layer.mudwallow": "Mud wallows",
  "layer.food": "Food zones",
  "layer.patrol": "AI patrol zones",
  "layer.region": "Region names",
  "layer.landmark": "Landmarks",
  "layers.title": "Map layers",
  "layers.zone_labels": "Zone name labels",

  "wp.title": "Waypoints",
  "wp.new": "New waypoint",
  "wp.add": "Add waypoint",
  "wp.remove": "Delete",
  "wp.rename": "Rename",
  "wp.name_prompt": "Waypoint name:",
  "wp.empty": "No waypoints yet. Right-click the map to add one.",
  "wp.distance": "{dir} · {dist}",
  "wp.here": "My position",
  "wp.confirm_delete": "Delete waypoint “{name}”?",

  "trail.title": "Travelled path",
  "trail.previous": "Previous session path",

  "btn.close": "Close",
  "btn.ok": "OK",
  "btn.cancel": "Cancel",
  "btn.save": "Save",

  "warn.exclusive_fullscreen":
    "The game is running in exclusive Fullscreen mode. The minimap cannot draw on top of it. " +
    "In the game go to Settings › Video and switch to “Windowed” or “Borderless Fullscreen”.",
  "warn.hotkey_failed":
    "The following hotkeys could not be registered because another app holds them:",
  "warn.no_data": "No map data on this machine yet. It needs to be downloaded once before use.",

  "hotkey.toggle_minimap": "Show/hide minimap",
  "hotkey.toggle_fullmap": "Open/close full map",
  "hotkey.toggle_click_through": "Toggle click-through",
  "hotkey.mark_here": "Mark current position",
  "hotkey.opacity_up": "Minimap more opaque",
  "hotkey.opacity_down": "Minimap more transparent",
  "hotkey.zoom_in": "Zoom view in",
  "hotkey.zoom_out": "Zoom view out",

  "settings.language": "Ngôn ngữ · Language",
  "settings.minimap": "Minimap",
  "settings.visible": "Show minimap",
  "settings.click_through": "Click-through (never blocks gameplay)",
  "settings.corner": "Anchor corner on the game window",
  "corner.top-left": "Top left",
  "corner.top-right": "Top right",
  "corner.bottom-left": "Bottom left",
  "corner.bottom-right": "Bottom right",
  "settings.size": "Size",
  "settings.margin": "Margin",
  "settings.opacity": "Opacity",
  "settings.radius": "View radius",
  "settings.hotkeys": "Hotkeys",
  "settings.hotkeys_hint":
    "Click a key field, then press the new combination. At least one modifier (Ctrl/Alt/Shift/Win) is required.",
  "settings.press_keys": "Press keys… (Esc to cancel)",
  "settings.hotkey_in_use": "This combination is held by another application",
  "settings.hotkey_duplicate": "Duplicates another hotkey in this app",
  "settings.hotkey_invalid": "Invalid combination — at least one modifier required",
  "settings.number_format": "Coordinate number format",
  "format.auto": "Auto-detect",
  "format.us": "US style — 1,234.5",
  "format.eu": "EU style — 1.234,5",
  "settings.data": "Data",
  "settings.open_trails": "Open trails folder",
  "settings.redownload": "Re-download map data",

  "firstrun.title": "Download map data",
  "firstrun.explain":
    "The app needs to download the basemap (~3 MB) and point data to your machine once. " +
    "Data is fetched straight from its sources instead of being bundled — it is a personal " +
    "copy on your machine, not a redistribution.",
  "firstrun.start": "Start download",
  "firstrun.downloading": "Downloading…",
  "firstrun.done": "Done! Opening the map…",
  "firstrun.partial":
    "The basemap downloaded but the point data failed. The map still works; " +
    "retry the data download from Settings later.",
  "firstrun.failed": "Download failed. Check your connection and try again.",
  "firstrun.retry": "Retry",
  "firstrun.continue": "Continue with the map",

  "update.available": "Update {version} available",
  "update.install": "Update now",
  "update.installing": "Downloading update…",
  "update.later": "Later",

  "credits.title": "Data sources",
  "credits.body":
    "Basemap: VulnonaMAP (Coco.N) — stitched from in-game captures. " +
    "Imagery copyright Afterthought LLC (The Isle). " +
    "Point data: VulnonaMAP, myislemap.com, wiredredman's Steam guide. " +
    "This app is not affiliated with Afterthought LLC.",
};
