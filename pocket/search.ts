import { spawnSync } from "child_process";
import * as fs from "fs";
import * as os from "os";
import * as path from "path";
import { EXPORT_ROOT } from "./paths";

const COLLECTION = "pocket";
// Transcripts are .txt, summaries are .md — qmd only globs **/*.md by default.
const MASK = "**/*.{md,txt}";
const CONFIG = path.join(os.homedir(), ".config", "qmd", "index.yml");

// The bare `qmd` name on npm is a dead placeholder, so the scoped package it is.
// `--index index` pins the global index: without it qmd walks up from cwd and
// silently uses a project-local .qmd index that knows nothing about pocket.
const QMD = ["-y", "@tobilu/qmd", "--index", "index"];

const run = ({ args, capture = false }: { args: string[]; capture?: boolean }) =>
  spawnSync("npx", [...QMD, ...args], {
    cwd: EXPORT_ROOT,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });

const isIndexed = () =>
  fs.existsSync(CONFIG) && /^ {2}pocket:$/m.test(fs.readFileSync(CONFIG, "utf8"));

// Adding the collection indexes it; updating re-scans an existing one. Embedding
// only touches documents whose vectors are missing, so this is cheap when warm.
export const reindex = () => {
  fs.mkdirSync(EXPORT_ROOT, { recursive: true });
  const setup = ["collection", "add", EXPORT_ROOT, "--name", COLLECTION, "--mask", MASK];
  run({ args: isIndexed() ? ["update"] : setup });
  run({ args: ["embed", "-c", COLLECTION] });
};

const ensureIndexed = () => {
  if (isIndexed()) return;
  console.log("First search — building the index (downloads local models once).\n");
  reindex();
};

type Hit = {
  score: number;
  file: string;
  line: number;
  snippet: string;
};

export type Recording = {
  folder: string;
  title: string;
  date: string;
  dir: string;
  score: number;
  hits: Hit[];
};

// Export folders are named <slug>-<YYYY-MM-DD>.
const FOLDER = /^(.*)-(\d{4}-\d{2}-\d{2})$/;
const VIRTUAL = /^qmd:\/\/pocket\/([^/]+)\/(.+)$/;

const describe = ({ folder }: { folder: string }) => {
  const match = FOLDER.exec(folder);
  if (!match) return { title: folder, date: "" };
  return { title: match[1].replace(/-/g, " "), date: match[2] };
};

// Every hit carries a diff-style "@@ -462,4 @@ (461 before…)" header line.
const trim = ({ snippet }: { snippet: string }) =>
  snippet.replace(/^@@[^\n]*\n/, "").replace(/\s+/g, " ").trim();

// One recording produces many chunk hits; fold them into a single result
// ranked by its best chunk, since the question is "which conversation?".
const group = ({ hits }: { hits: Hit[] }) => {
  const byFolder = new Map<string, Recording>();
  for (const raw of hits) {
    const match = VIRTUAL.exec(raw.file);
    if (!match) continue;
    const folder = match[1];
    const hit = { ...raw, file: match[2], snippet: trim({ snippet: raw.snippet }) };
    const found = byFolder.get(folder);
    if (found) {
      found.score = Math.max(found.score, hit.score);
      found.hits.push(hit);
      continue;
    }
    byFolder.set(folder, {
      folder,
      ...describe({ folder }),
      dir: path.join(EXPORT_ROOT, folder),
      score: hit.score,
      hits: [hit],
    });
  }
  return [...byFolder.values()].sort((a, b) => b.score - a.score);
};

export const searchRecordings = ({
  query,
  limit = 5,
  rerank = true,
}: {
  query: string;
  limit?: number;
  rerank?: boolean;
}) => {
  ensureIndexed();

  // Ask for more chunks than recordings wanted — several usually share a folder.
  const args = ["query", query, "-c", COLLECTION, "-n", String(limit * 4), "--format", "json"];
  if (!rerank) args.push("--no-rerank");

  const res = run({ args, capture: true });
  if (res.status !== 0) return [];

  const hits: Hit[] = JSON.parse(res.stdout || "[]");
  return group({ hits }).slice(0, limit);
};
