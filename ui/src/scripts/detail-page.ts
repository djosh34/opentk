import { el, requireElement } from "../lib/dom";
import { formatDate, formatValue, humanizeKey, presentValue } from "../lib/format";
import {
  API_BASE_URL,
  documentContent,
  entityDetail,
  type DocumentContent,
  type EntityDetail,
  type RelationDetail,
} from "../lib/opentk";

const status = requireElement("#status", HTMLParagraphElement);
const detail = requireElement("#detail", HTMLElement);
const params = new URL(location.href).searchParams;
const apiPath = params.get("api_url");
const query = params.get("q")?.trim() ?? "";

if (!apiPath) {
  status.textContent = "Geen detail gekozen.";
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
    detail.replaceChildren(renderDetail(entity, path, content));
  } catch (error) {
    console.error(error);
    status.textContent = "Dit item kan nu niet geladen worden.";
    detail.replaceChildren(shell("Laden mislukt."));
  }
}

function renderDetail(entity: EntityDetail, path: string, content: DocumentContent | null): HTMLElement {
  const blocks = [
    el("a", "mb-4 inline-flex rounded-md border border-slate-300 px-3 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-50", ["Terug naar resultaten"], {
      href: query ? `/?q=${encodeURIComponent(query)}` : "/",
    }),
    el("h2", "text-2xl font-semibold leading-tight text-slate-950", [detailTitle(entity)]),
    el("div", "mt-3 flex flex-wrap items-center gap-2 text-xs font-medium text-slate-500", [
      pill(entity.metadata.category),
      entity.metadata.source_id,
      entity.metadata.source_updated_at ? formatDate(entity.metadata.source_updated_at) : null,
    ]),
    el("div", "mt-4 flex flex-wrap gap-2", actionLinks(entity, path, content)),
    fieldSection("Gegevens", entity.fields),
    content ? documentSection(content) : null,
    entity.relations?.length ? relationsSection(entity.relations) : null,
  ];

  return el("article", "rounded-lg border border-slate-200 bg-white p-4 shadow-sm sm:p-5", blocks);
}

function actionLinks(entity: EntityDetail, path: string, content: DocumentContent | null): HTMLAnchorElement[] {
  const externalUrl = externalSourceUrl(entity, content);
  const links = [actionLink(`${API_BASE_URL}${path}`, "API")];
  if (externalUrl) {
    links.push(actionLink(externalUrl, "Bron"));
  }
  return links;
}

function fieldSection(title: string, fields: Record<string, unknown>): HTMLElement {
  const rows = Object.entries(fields)
    .filter(([, value]) => presentValue(value))
    .map(([key, value]) => el("div", "border-t border-slate-100 pt-3", [
      el("dt", "text-xs font-medium text-slate-500", [humanizeKey(key)]),
      el("dd", "mt-1 break-words text-sm leading-6 text-slate-800", [formatValue(value)]),
    ]));

  return el("section", "mt-6", [
    el("h3", "text-sm font-semibold uppercase tracking-normal text-slate-500", [title]),
    el("dl", "mt-3 grid gap-3", rows),
  ]);
}

function documentSection(document: DocumentContent): HTMLElement {
  return el("section", "mt-6 border-t border-slate-100 pt-5", [
    el("h3", "text-sm font-semibold uppercase tracking-normal text-slate-500", ["Inhoud"]),
    el("p", "mt-2 text-sm text-slate-600", [
      `${document.content.extraction_status} / ${document.content.validation_status}`,
    ]),
    el("div", "mt-4 max-h-[32rem] overflow-auto rounded-md border border-slate-200 bg-slate-50 p-4 text-sm leading-6 whitespace-pre-wrap text-slate-800", [
      document.content.extracted_text
        ?? document.content.extraction_error
        ?? "Geen geëxtraheerde tekst beschikbaar.",
    ]),
  ]);
}

function relationsSection(relations: RelationDetail[]): HTMLElement {
  return el("section", "mt-6 border-t border-slate-100 pt-5", [
    el("h3", "text-sm font-semibold uppercase tracking-normal text-slate-500", ["Relaties"]),
    el("ol", "mt-3 grid gap-2", relations.map((relation) => (
      el("li", "rounded-md bg-slate-50 p-3 text-sm text-slate-700", [
        `${relation.relation_name}: ${relation.source_category}/${relation.source_id} -> ${relation.target_category}/${relation.target_id}`,
      ])
    ))),
  ]);
}

function actionLink(href: string, label: string): HTMLAnchorElement {
  return el("a", "rounded-md border border-slate-300 px-3 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-50", [label], {
    href,
    target: "_blank",
    rel: "noreferrer",
  });
}

function shell(message: string): HTMLElement {
  return el("div", "rounded-lg border border-slate-200 bg-white p-5 text-sm text-slate-600 shadow-sm", [message]);
}

function pill(text: string): HTMLSpanElement {
  return el("span", "rounded bg-slate-100 px-2 py-1 text-slate-700", [text]);
}

function detailTitle(entity: EntityDetail): string {
  for (const key of ["titel", "onderwerp", "naam", "achternaam", "nummer", "document_nummer"]) {
    const value = entity.fields[key];
    if (typeof value === "string" && value.trim()) {
      return value;
    }
  }
  return `${entity.metadata.category} ${entity.metadata.source_id}`;
}

function externalSourceUrl(entity: EntityDetail, content: DocumentContent | null): string | null {
  for (const key of ["enclosure_url", "url"]) {
    const value = entity.fields[key];
    if (typeof value === "string" && /^https?:\/\//.test(value)) {
      return value;
    }
  }
  return content?.content.selected_source_url ?? null;
}
