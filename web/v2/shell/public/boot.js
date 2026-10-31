// The boot script of the shell (spec 021, REQ-021-1): a file (CSP `script-src 'self'`, no
// inline script), run by the head before the first paint, so that the skeleton of
// index.html takes the geometry the frame will mount in. It reads what the frame keeps
// (src/frame/settings.ts, src/time/index.ts) and marks the root: theme, density and motion
// as the settings apply them; the sidebar collapsed; a dated URL (the band); the right
// panel open and its width. Nothing else: the application applies the same on its start.
(() => {
  const root = document.documentElement;
  const read = (key) => {
    try {
      return globalThis.localStorage?.getItem(key) ?? null;
    } catch {
      return null;
    }
  };
  const theme = read("kartenn.theme");
  if (theme === "light" || theme === "dark") root.dataset.theme = theme;
  if (read("kartenn.density") === "comfortable") root.dataset.density = "comfortable";
  if (read("kartenn.motion") === "reduce") root.dataset.motion = "reduce";
  if (read("kartenn.sidebar") === "collapsed") root.dataset.bootSidebar = "collapsed";

  // A date as the frame takes it: an instant of RFC 3339 in UTC; any other is absent.
  const INSTANT = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$/;
  const query = new URLSearchParams(location.search);
  const dated = ["valid_at", "as_of"].some((name) => {
    const value = query.get(name);
    return value !== null && INSTANT.test(value) && !Number.isNaN(Date.parse(value));
  });
  if (dated) root.dataset.bootDated = "";

  let panel = null;
  try {
    panel = JSON.parse(read("kartenn.panel") ?? "null");
  } catch {
    panel = null;
  }
  if (panel?.open === true) {
    root.dataset.bootPanel = "open";
    if (typeof panel.width === "number" && Number.isFinite(panel.width))
      root.style.setProperty("--k-boot-panel-width", `${panel.width}px`);
  }
})();
