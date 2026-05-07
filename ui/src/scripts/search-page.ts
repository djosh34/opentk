import { el, requireElement } from "../lib/dom";
import { API_BASE_URL, search, type SearchResponse, type SearchResult } from "../lib/opentk";
import { formatDate } from "../lib/format";
import { rememberCurrentScroll, restoreCurrentScroll } from "./history-scroll";

const form = requireElement("#search-form", HTMLFormElement);
const input = requireElement("#search-input", HTMLInputElement);
const status = requireElement("#status", HTMLParagraphElement);
const results = requireElement("#results", HTMLOListElement);
const query = new URL(location.href).searchParams.get("q")?.trim() ?? "";

if ("scrollRestoration" in history) {
  history.scrollRestoration = "manual";
}

input.value = query;
form.addEventListener("submit", () => {
  input.value = input.value.trim();
});
window.addEventListener("pagehide", rememberCurrentScroll);

if (query) {
  void loadResults(query);
}

async function loadResults(searchQuery: string): Promise<void> {
  const controller = new AbortController();
  status.textContent = "Zoeken...";
  results.replaceChildren();

  try {
    renderResults(await search(searchQuery, controller.signal));
  } catch (error) {
    console.error(error);
    status.textContent = "Zoeken lukt nu niet. Probeer het later opnieuw.";
  }
}

function renderResults(data: SearchResponse): void {
  if (data.items.length === 0) {
    status.textContent = `Geen resultaten voor "${data.query}".`;
    return;
  }

  const count = data.estimated_total_hits ?? data.items.length;
  status.textContent = `${count} resultaat${count === 1 ? "" : "en"} voor "${data.query}".`;
  results.replaceChildren(...data.items.map(resultItem));
  restoreCurrentScroll();
}

function resultItem(item: SearchResult): HTMLLIElement {
  const link = el("a", "outline-none hover:underline focus:underline", [item.title], {
    href: detailHref(item.api_url),
  });
  const meta = el("div", "mb-2 flex flex-wrap items-center gap-x-2 gap-y-1 text-xs font-medium text-slate-500", [
    pill(item.entity_kind),
    item.source_category,
    item.document_number ? `Nr. ${item.document_number}` : null,
  ]);
  const footer = el("div", "mt-3 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-slate-500", [
    item.date ? el("time", "", [formatDate(item.date)], { datetime: item.date }) : null,
    el("a", "font-medium text-slate-700 underline underline-offset-2", ["API"], {
      href: `${API_BASE_URL}${item.api_url}`,
    }),
  ]);

  return el("li", "rounded-lg border border-slate-200 bg-white p-4 shadow-sm", [
    meta,
    el("h2", "text-lg font-semibold leading-snug text-slate-950", [link]),
    el("p", "mt-2 text-sm leading-6 text-slate-700", [
      item.summary ?? bestSnippetText(item) ?? "Geen samenvatting beschikbaar.",
    ]),
    footer,
  ]);
}

function detailHref(apiPath: string): string {
  const url = new URL("/detail", location.origin);
  url.searchParams.set("api_url", apiPath);
  if (query) {
    url.searchParams.set("q", query);
  }
  return `${url.pathname}${url.search}`;
}

function pill(text: string): HTMLSpanElement {
  return el("span", "rounded bg-slate-100 px-2 py-1 text-slate-700", [text]);
}

function bestSnippetText(item: SearchResult): string | null {
  return item.snippets.find((snippet) => snippet.text.trim().length > 0)?.text ?? null;
}
