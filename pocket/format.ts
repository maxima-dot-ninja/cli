import type { Recording } from "./search";

const slugify = ({ text }: { text: string }) =>
  text
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");

export const recordingDate = ({ rec }: { rec: any }) => {
  const date = new Date(rec.recording_at || rec.created_at || "");
  if (isNaN(date.getTime())) return "unknown-date";
  return date.toISOString().slice(0, 10);
};

export const recordingLabel = ({ rec }: { rec: any }) =>
  `${recordingDate({ rec })}  ${rec.title || "(untitled)"}`;

export const folderName = ({ rec }: { rec: any }) => {
  const slug = slugify({ text: rec.title || "untitled" });
  return `${slug.slice(0, 60) || "untitled"}-${recordingDate({ rec })}`;
};

const transcriptLine = ({ seg }: { seg: any }) => {
  const speaker = seg.speaker_name || seg.speaker || "";
  const text = seg.text || seg.content || "";
  if (!speaker) return text;
  return `${speaker}: ${text}`;
};

// Transcript arrives as { metadata, segments, text }.
export const transcriptText = ({ transcript }: { transcript: any }) => {
  const segments = transcript?.segments || transcript;
  if (typeof segments === "string") return segments;
  if (!Array.isArray(segments)) return "";
  return segments.map((seg: any) => transcriptLine({ seg })).join("\n");
};

const actionLines = ({ action }: { action: any }) => {
  const done = action.isCompleted ? "x" : " ";
  const due = action.dueDate ? ` (due ${action.dueDate})` : "";
  const lines = [`- [${done}] ${action.label}${due}`];
  if (action.context) lines.push(`  - ${action.context}`);
  return lines;
};

// Each summarization carries its content under v2 (summary.markdown, actionItems).
const summaryBlock = ({ summ }: { summ: any }) => {
  const content = summ.v2?.summary?.markdown || "";
  const actions = summ.v2?.actionItems?.actions || [];
  const items = actions.flatMap((action: any) => actionLines({ action }));
  const section = items.length ? ["## Action items", "", ...items, ""] : [];
  return [content, "", ...section];
};

const hitLine = ({ hit }: { hit: any }) => {
  const quote = hit.snippet.length > 220 ? `${hit.snippet.slice(0, 220)}…` : hit.snippet;
  return `      ${hit.file}:${hit.line}  ${quote}`;
};

// Best two chunks per recording — enough to judge relevance without a wall of text.
const resultBlock = ({ result, rank }: { result: Recording; rank: number }) => {
  const percent = `${Math.round(result.score * 100)}%`.padStart(4);
  const date = result.date || "unknown date";
  return [
    `${String(rank).padStart(2)}. ${percent}  ${result.title}  ·  ${date}`,
    `      ${result.dir}`,
    ...result.hits.slice(0, 2).map((hit) => hitLine({ hit })),
    "",
  ];
};

export const searchResultsText = ({ results }: { results: Recording[] }) => {
  if (!results.length) return "\nNo matching conversations.\n";
  const blocks = results.flatMap((result, i) => resultBlock({ result, rank: i + 1 }));
  return ["", ...blocks, `${results.length} matching recordings`].join("\n");
};

export const summaryMarkdown = ({ rec }: { rec: any }) => {
  const meta = [
    `# ${rec.title || "Untitled recording"}`,
    "",
    `- **Date:** ${recordingDate({ rec })}`,
    `- **Duration:** ${rec.duration || "?"}s`,
    `- **Recording ID:** ${rec.id}`,
    "",
  ];
  const blocks = Object.values(rec.summarizations || {}).flatMap((summ: any) =>
    summaryBlock({ summ })
  );
  if (!blocks.length) blocks.push("_No summary available._");
  return [...meta, ...blocks].join("\n");
};
