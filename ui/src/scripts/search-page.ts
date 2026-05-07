import { el, requireElement } from "../lib/dom";
import { search, type SearchResponse, type SearchResult } from "../lib/opentk";
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
  status.textContent = `${formatCount(count)} ${count === 1 ? "resultaat" : "resultaten"} voor "${data.query}".`;
  results.replaceChildren(...data.items.map(resultItem));
  restoreCurrentScroll();
}

function resultItem(item: SearchResult): HTMLLIElement {
  const href = detailHref(item);
  const summary = searchSummary(item);
  const meta = el("div", "flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-stone-600", [
    readableKind(item),
    item.date ? el("time", "", [formatDate(item.date)], { datetime: item.date }) : null,
  ]);
  const arrow = el("span", "grid h-9 w-9 flex-none place-items-center rounded-full border border-stone-200 text-stone-500 transition group-hover:border-lime-700 group-hover:text-lime-800", [
    "→",
  ]);
  const link = el("a", "group block rounded-lg border border-stone-200 bg-white p-4 text-left shadow-sm shadow-stone-950/5 outline-none transition hover:-translate-y-0.5 hover:border-lime-700 hover:shadow-md focus-visible:border-stone-950 focus-visible:ring-3 focus-visible:ring-lime-700/20 sm:p-5", [
    el("div", "flex items-start justify-between gap-4", [
      el("div", "min-w-0", [
        meta,
        el("h2", "mt-2 text-lg font-semibold leading-snug text-stone-950 sm:text-xl", [item.title]),
      ]),
      arrow,
    ]),
    summary ? el("p", "mt-3 max-w-3xl text-sm leading-6 text-stone-700", [summary]) : null,
  ], { href });

  return el("li", "", [link]);
}

function detailHref(item: SearchResult): string {
  const url = new URL("/detail", location.origin);
  url.searchParams.set("api", item.api_url);
  if (query) {
    url.searchParams.set("q", query);
  }
  return `${url.pathname}${url.search}`;
}

function searchSummary(item: SearchResult): string | null {
  if (item.summary?.trim()) {
    return cleanText(item.summary);
  }
  const readableSnippet = item.snippets
    .map((snippet) => snippet.text)
    .map(cleanMetadataSnippet)
    .find((text) => text.length > 0 && text.toLowerCase() !== item.title.toLowerCase());
  return readableSnippet ?? null;
}

function cleanMetadataSnippet(text: string): string {
  const decoded = decodeHtml(text).replace(/\s+/g, " ").trim();
  if (!decoded.includes(":")) {
    return cleanText(decoded);
  }
  const fields = Object.fromEntries([...decoded.matchAll(/([\p{L}_]+):\s*([^:]+?)(?=\s+[\p{L}_]+:\s|$)/gu)]
    .map((match) => [match[1], cleanText(match[2])]));
  const preferred = [
    fields.soort,
    fields.status,
    fields.voortouwnaam,
    fields.actor_naam,
    fields.functie,
    fields.actor_fractie,
    fields.vergaderjaar ? `Vergaderjaar ${fields.vergaderjaar}` : null,
  ].filter(Boolean);
  if (preferred.length > 0) {
    return preferred.join(" · ");
  }
  return "";
}

function cleanText(text: string): string {
  return text
    .replace(/S-\d(?:-\d+)+/g, "")
    .replace(/\b[0-9a-f]{8}-[0-9a-f-]{27,}\b/gi, "")
    .replace(/\s+/g, " ")
    .trim();
}

function decodeHtml(text: string): string {
  const textarea = document.createElement("textarea");
  textarea.innerHTML = text;
  return textarea.value;
}

function formatCount(count: number): string {
  return new Intl.NumberFormat("nl-NL").format(count);
}

function readableKind(item: SearchResult): string {
  const category = item.source_category.toLowerCase();
  if (category.includes("document")) {
    if (category.includes("actor")) {
      return "Documentbetrokkene";
    }
    if (category.includes("versie")) {
      return "Documentversie";
    }
    return "Document";
  }
  if (category === "zaak") {
    return "Dossier";
  }
  if (category.includes("zaakactor")) {
    return "Dossierbetrokkene";
  }
  if (category.includes("activiteit")) {
    return "Vergadering";
  }
  if (category.includes("persoon")) {
    return "Persoon";
  }
  if (category.includes("agendapunt")) {
    return "Agendapunt";
  }
  return item.entity_kind === "Other" ? humanizeSourceCategory(item.source_category) : item.entity_kind;
}

function humanizeSourceCategory(category: string): string {
  return category
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replaceAll("_", " ");
}
