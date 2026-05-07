const SCROLL_STATE_KEY = "opentkScroll";

type ScrollEntry = {
  href: string;
  x: number;
  y: number;
};

type HistoryState = Record<string, unknown> & {
  [SCROLL_STATE_KEY]?: ScrollEntry;
};

export function rememberCurrentScroll(): void {
  const nextState = currentHistoryState();
  nextState[SCROLL_STATE_KEY] = {
    href: location.href,
    x: window.scrollX,
    y: window.scrollY,
  };
  history.replaceState(nextState, "", location.href);
}

export function restoreCurrentScroll(): void {
  const entry = currentHistoryState()[SCROLL_STATE_KEY];
  if (!entry || entry.href !== location.href) {
    return;
  }

  requestAnimationFrame(() => {
    window.scrollTo(entry.x, entry.y);
  });
}

function currentHistoryState(): HistoryState {
  if (history.state && typeof history.state === "object" && !Array.isArray(history.state)) {
    return { ...history.state };
  }
  return {};
}
