// English translations. Typed as Record<MsgKey, string>: a missing key is a
// COMPILE error, so the two languages cannot drift apart.

import type { MsgKey } from "./vi";

export const en: Record<MsgKey, string> = {
  "app.title": "The Isle Map",
  "app.minimap_title": "Minimap",
  "app.fullmap_title": "Gateway Map",

  "tab.map": "Map",
  "tab.dino": "Your Dino",
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
  "hotkey.reload_ui": "Reload the UI (if it freezes)",
  "hotkey.zoom_out": "Zoom view out",

  "settings.language": "Ngôn ngữ · Language",
  "settings.minimap": "Minimap",
  "settings.visible": "Show minimap",
  "settings.require_game": "Only show while you are in the game (hides on Alt-Tab)",
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

  "dino.title": "Your dino",
  "dino.explain":
    "Reads your OWN dino's info from the server's IslePilot panel (growth, health, hunger, " +
    "thirst, Prime progress). It is just an HTTPS connection to the server's website — " +
    "nothing touches the game, anti-cheat safe.",
  "dino.server": "Server",
  "dino.login": "Sign in with Steam",
  "dino.login_wait": "Waiting for you to sign in in the window that just opened…",
  "dino.login_failed": "Sign-in did not complete. Try again.",
  "dino.logged_in": "Signed in",
  "dino.logout": "Sign out",
  "dino.auth_expired": "Your session expired — please sign in again.",
  "dino.supported_servers":
    "Works with any IslePilot-powered server — xxx.islepilot.eu or islepilot.eu/p/server-name. " +
    "See the Guide tab for examples and a step-by-step walkthrough.",
  "dino.manual_cookie": "Paste your session cookie",
  "dino.manual_cookie_hint":
    "Open the server page in your browser and sign in with Steam. Press F12 → " +
    "Application tab (Chrome) or Storage (Firefox) → Cookies → pick the server domain → " +
    "find the cookie named islepilot_player and paste its Value here. Keep this string " +
    "secret like a password.",
  "dino.cancel_login": "Cancel sign-in",
  "dino.manual_cookie_save": "Verify & save cookie",
  "dino.manual_cookie_checking": "Checking cookie…",
  "dino.manual_cookie_bad":
    "Cookie invalid or session not signed in — double-check the pasted string.",
  "dino.server_settings": "Server settings",
  "dino.live_map_yes": "This server has a live map — your position updates automatically",
  "dino.live_map_checking": "Checking whether this server has a live map…",
  "dino.enabled": "Track dino info",
  "dino.interval": "Update frequency",
  "dino.overlay_panel": "Show stats strip under the minimap",
  "dino.use_map_position":
    "Auto position from the server's live map (instead of manual coordinate copying)",
  "dino.rules_note":
    "⚠ Ask the server admins before using this routinely — some servers have their own " +
    "rules about third-party tools. Everything shown is your own data, served by the " +
    "server's own panel.",
  "dino.growth": "Growth",
  "dino.health": "Health",
  "dino.hunger": "Hunger",
  "dino.thirst": "Thirst",
  "dino.prime": "Prime progress",
  "dino.online": "Online",
  "dino.offline": "Offline",
  "dino.updated": "Updated {time}",
  "dino.no_data": "No data yet — enable tracking and wait for the first update.",
  "dino.fetch_error": "Panel connection error:",
  "dino.layout_changed":
    "IslePilot just deployed a new version — if numbers look wrong, their markup may have " +
    "changed and the app needs an update.",
  "dino.map_disabled": "The live map is disabled on this server.",
  "dino.crashed":
    "The Your Dino section hit an error and was isolated — the map and other features are unaffected.",
  "map.crashed":
    "The map hit a display error. Click Retry, or press F5 to reload the whole app.",
  "btn.retry": "Retry",

  "update.available": "Update {version} available",
  "update.install": "Update now",
  "update.installing": "Downloading update…",
  "update.later": "Later",

  "footer.developed_by": "Developed by",
  "footer.donate": "Donate",
  "footer.reload_hint": "If the app breaks, press F5 or Ctrl+Alt+R to reload",
  "donate.title": "Support the author",
  "donate.hint": "Scan the VietQR code with your banking app, or transfer manually:",
  "donate.copy_stk": "Copy account number",
  "donate.copied": "Copied!",
  "donate.thanks": "Thank you for your support! ❤",

  "credits.title": "Data sources",
  "credits.body":
    "Basemap: VulnonaMAP (Coco.N) — stitched from in-game captures. " +
    "Imagery copyright Afterthought LLC (The Isle). " +
    "Point data: VulnonaMAP, myislemap.com, wiredredman's Steam guide. " +
    "This app is not affiliated with Afterthought LLC.",
};
