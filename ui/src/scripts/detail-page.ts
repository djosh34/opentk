import { el, requireElement } from "../lib/dom";
import { formatDate, formatValue, humanizeKey, presentValue } from "../lib/format";
import {
  documentContent,
  entityDetail,
  type DocumentContent,
  type EntityDetail,
} from "../lib/opentk";

const status = requireElement("#status", HTMLParagraphElement);
const detail = requireElement("#detail", HTMLElement);
const params = new URL(location.href).searchParams;
const itemKey = params.get("item");
const query = params.get("q")?.trim() ?? "";
const apiPath = itemKey ? detailTarget(itemKey, query) : null;

if (!apiPath) {
  status.textContent = "Open dit item opnieuw vanuit de zoekresultaten.";
} else {
  void loadDetail(apiPath);
}

async function loadDetail(path: string): Promise<void> {
  const controller = new AbortController();
  detail.replaceChildren(shell("Laden..."));

  try {
    const entity = await entityDetail(path, controller.signal);
    const content = entity.metadata.category === "Document"
      ? await documentContent(path, controller.signal)
      : null;
    status.textContent = "";
    detail.replaceChildren(renderDetail(entity, content));
  } catch (error) {
    console.error(error);
    status.textContent = "Dit item kan nu niet geladen worden.";
    detail.replaceChildren(shell("Laden mislukt."));
  }
}

function renderDetail(entity: EntityDetail, content: DocumentContent | null): HTMLElement {
  const sourceUrl = externalSourceUrl(entity, content);
  const blocks = [
    el("div", "mb-5 flex flex-wrap items-center justify-between gap-3", [
      el("a", "inline-flex items-center gap-2 rounded-md border border-stone-300 bg-white px-3 py-2 text-sm font-semibold text-stone-700 transition hover:border-stone-950 hover:text-stone-950", ["←", "Resultaten"], {
        href: query ? `/?q=${encodeURIComponent(query)}` : "/",
      }),
      sourceUrl ? actionLink(sourceUrl, "Bron openen") : null,
    ]),
    el("article", "grid gap-6 lg:grid-cols-[minmax(0,1fr)_18rem]", [
      el("section", "min-w-0", [
        el("div", "mb-4", [
          el("div", "mb-3 flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-stone-600", detailMeta(entity).map(metaItem)),
          el("h2", "max-w-4xl text-2xl font-semibold leading-tight text-stone-950 sm:text-4xl", [detailTitle(entity)]),
        ]),
        content || sourceUrl ? documentSection(entity, content, sourceUrl) : fieldSection("Informatie", entity.fields, "main"),
      ]),
      el("aside", "lg:pt-15", [
        fieldSection("Details", entity.fields, "aside"),
      ]),
    ]),
  ];

  return el("div", "", blocks);
}

function actionLink(href: string, label: string): HTMLAnchorElement {
  return el("a", "inline-flex items-center rounded-md bg-stone-950 px-3 py-2 text-sm font-semibold text-white transition hover:bg-lime-800", [label], {
    href,
    target: "_blank",
    rel: "noreferrer",
  });
}

function fieldSection(title: string, fields: Record<string, unknown>, variant: "main" | "aside"): HTMLElement | null {
  const rows = relevantFields(fields, variant)
    .map(([key, value]) => el("div", "border-t border-stone-200 py-3 first:border-t-0 first:pt-0", [
      el("dt", "text-xs font-semibold uppercase tracking-normal text-stone-500", [humanizeLabel(key)]),
      el("dd", "mt-1 break-words text-sm leading-6 text-stone-800", [formatCleanValue(key, value)]),
    ]));

  if (rows.length === 0) {
    return null;
  }

  return el("section", variant === "main" ? "rounded-lg border border-stone-200 bg-white p-5 shadow-sm shadow-stone-950/5" : "rounded-lg border border-stone-200 bg-white p-4 shadow-sm shadow-stone-950/5", [
    el("h3", "mb-3 text-sm font-semibold text-stone-950", [title]),
    el("dl", "", rows),
  ]);
}

function documentSection(entity: EntityDetail, document: DocumentContent | null, sourceUrl: string | null): HTMLElement {
  const text = document?.content.extracted_text?.trim();
  const error = document?.content.extraction_error?.trim();
  const sourceType = contentType(entity, document);
  const preview = sourceUrl
    ? mediaPreview(sourceUrl, sourceType)
    : null;

  return el("section", "grid gap-4", [
    preview,
    text ? el("div", "rounded-lg border border-stone-200 bg-white p-5 shadow-sm shadow-stone-950/5", [
      el("h3", "mb-3 text-sm font-semibold text-stone-950", ["Tekst"]),
      el("div", "max-h-[34rem] overflow-auto whitespace-pre-wrap text-sm leading-7 text-stone-800", [text]),
    ]) : null,
    !preview && !text ? el("div", "rounded-lg border border-stone-200 bg-white p-5 text-sm leading-6 text-stone-700 shadow-sm shadow-stone-950/5", [
      error || "Voor dit item is nog geen voorbeeld of tekst beschikbaar.",
    ]) : null,
  ]);
}

function mediaPreview(sourceUrl: string, sourceType: string | null): HTMLElement {
  if (sourceType?.includes("pdf") || sourceUrl.toLowerCase().includes(".pdf") || sourceUrl.includes("/Resources/")) {
    return el("div", "grid min-h-[28rem] place-items-center rounded-lg border border-stone-200 bg-white p-6 text-center shadow-sm shadow-stone-950/5", [
      el("div", "max-w-md", [
        el("div", "mx-auto mb-5 grid h-20 w-16 place-items-center rounded-md border border-stone-300 bg-stone-50 text-sm font-semibold text-stone-600", ["PDF"]),
        el("h3", "text-lg font-semibold text-stone-950", ["Voorbeeld niet beschikbaar"]),
        el("p", "mt-2 text-sm leading-6 text-stone-600", [
          "De Tweede Kamer-bron blokkeert insluiten op andere websites. Open de bron om het document zelf te bekijken.",
        ]),
        el("div", "mt-5", [actionLink(sourceUrl, "Bron openen")]),
      ]),
    ]);
  }

  if (sourceType?.startsWith("image/")) {
    return el("div", "rounded-lg border border-stone-200 bg-white p-3 shadow-sm shadow-stone-950/5", [
      el("img", "max-h-[72dvh] w-full rounded-md object-contain", [], {
        src: sourceUrl,
        alt: "Documentvoorbeeld",
      }),
    ]);
  }

  return el("div", "rounded-lg border border-stone-200 bg-white p-5 text-sm leading-6 text-stone-700 shadow-sm shadow-stone-950/5", [
    "Dit bestand kan niet direct worden weergegeven. Open de bron om het document te bekijken.",
  ]);
}

function shell(message: string): HTMLElement {
  return el("div", "rounded-lg border border-stone-200 bg-white p-5 text-sm text-stone-600 shadow-sm shadow-stone-950/5", [message]);
}

function detailTitle(entity: EntityDetail): string {
  for (const key of ["titel", "onderwerp", "naam", "achternaam", "nummer", "document_nummer"]) {
    const value = entity.fields[key];
    if (typeof value === "string" && value.trim()) {
      return cleanValue(value);
    }
  }
  return readableCategory(entity.metadata.category);
}

function detailMeta(entity: EntityDetail): Array<Node | string | null> {
  const fields = entity.fields;
  return [
    readableCategory(entity.metadata.category),
    typeof fields.soort === "string" ? fields.soort : null,
    typeof fields.status === "string" ? fields.status : null,
    typeof fields.datum === "string" ? el("time", "", [formatDate(fields.datum)], { datetime: fields.datum }) : null,
    typeof fields.aanvangstijd === "string" ? el("time", "", [formatDate(fields.aanvangstijd)], { datetime: fields.aanvangstijd }) : null,
  ];
}

function metaItem(value: Node | string | null): HTMLElement | null {
  if (!value) {
    return null;
  }
  return el("span", "", [value]);
}

function externalSourceUrl(entity: EntityDetail, content: DocumentContent | null): string | null {
  for (const key of ["enclosure_url", "url"]) {
    const value = entity.fields[key];
    if (typeof value === "string" && /^https?:\/\//.test(value)) {
      return value;
    }
  }
  const selectedUrl = content?.content.selected_source_url;
  return selectedUrl && /^https?:\/\//.test(selectedUrl) ? selectedUrl : null;
}

function contentType(entity: EntityDetail, content: DocumentContent | null): string | null {
  const fieldType = entity.fields.content_type;
  return typeof fieldType === "string" ? fieldType : content?.content.selected_source_content_type ?? null;
}

function relevantFields(fields: Record<string, unknown>, variant: "main" | "aside"): Array<[string, unknown]> {
  const keys = variant === "main"
    ? ["noot", "omschrijving", "samenvatting", "tekst", "onderwerp"]
    : ["soort", "status", "datum", "aanvangstijd", "eindtijd", "vergaderjaar", "organisatie", "voortouwnaam", "actor_naam", "functie", "actor_fractie"];

  const seen = new Set<string>();
  return keys
    .filter((key) => {
      if (seen.has(key)) {
        return false;
      }
      seen.add(key);
      return presentValue(fields[key]) && !isTechnicalKey(key);
    })
    .map((key) => [key, fields[key]]);
}

function humanizeLabel(key: string): string {
  const labels: Record<string, string> = {
    aanvangstijd: "Begint",
    actor_fractie: "Fractie",
    actor_naam: "Naam",
    eindtijd: "Eindigt",
    noot: "Notitie",
    soort: "Type",
    voortouwnaam: "Commissie",
  };
  return labels[key] ?? humanizeKey(key);
}

function formatCleanValue(key: string, value: unknown): string {
  if (typeof value === "string" && /tijd|datum/.test(key)) {
    return formatDate(value);
  }
  return cleanValue(formatValue(value));
}

function cleanValue(value: string): string {
  return value
    .replace(/<[^>]*>/g, " ")
    .replace(/S-\d(?:-\d+)+/g, "")
    .replace(/\b[0-9a-f]{8}-[0-9a-f-]{27,}\b/gi, "")
    .replace(/\s+/g, " ")
    .trim();
}

function isTechnicalKey(key: string): boolean {
  return /(^|_)(id|url|token|deleted|ordinal|vrs|sid|nummer)$/.test(key) || key.includes("source");
}

function readableCategory(category: string): string {
  const lower = category.toLowerCase();
  if (lower.includes("document")) {
    return "Document";
  }
  if (lower.includes("activiteit")) {
    return "Vergadering";
  }
  if (lower.includes("agendapunt")) {
    return "Agendapunt";
  }
  return category;
}

function detailTarget(item: string, searchQuery: string): string | null {
  const stored = sessionStorage.getItem(`opentk-detail-targets:${searchQuery}`);
  if (!stored) {
    return null;
  }
  const targets = JSON.parse(stored) as Record<string, unknown>;
  const path = targets[item];
  return typeof path === "string" && path.startsWith("/") ? path : null;
}
