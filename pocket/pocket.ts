#!/usr/bin/env bun
import { appKey, listRecordings } from "./api";
import { exportRecording } from "./export";
import { recordingLabel, searchResultsText } from "./format";
import { EXPORT_ROOT } from "./paths";
import { reindex, searchRecordings } from "./search";
import { prompt, select } from "./select";

const FLAGS = new Set(["--json", "--fast"]);
const OPTS = new Set(["--search", "-n"]);

const parse = ({ argv }: { argv: string[] }) => {
  const args: string[] = [];
  const opts: Record<string, string> = {};
  const flags = new Set<string>();

  for (let i = 0; i < argv.length; i++) {
    const token = argv[i];
    if (FLAGS.has(token)) flags.add(token);
    else if (OPTS.has(token)) opts[token] = argv[++i] || "";
    else args.push(token);
  }
  return { args, opts, flags };
};

const cli = parse({ argv: process.argv.slice(2) });

const list = async () => {
  const recs = await listRecordings();
  console.log("");
  for (const rec of recs) console.log(`${rec.id}  ${recordingLabel({ rec })}`);
  console.log(`\n${recs.length} recordings`);
};

const search = ({ query }: { query: string }) => {
  const results = searchRecordings({
    query,
    limit: Number(cli.opts["-n"]) || 5,
    rerank: !cli.flags.has("--fast"),
  });

  if (cli.flags.has("--json")) return console.log(JSON.stringify(results, null, 2));
  console.log(searchResultsText({ results }));
};

const searchInteractive = async () => {
  const query = await prompt({ title: "\nWhat are you looking for?" });
  if (query) search({ query });
};

const exportOne = async () => {
  const recs = await listRecordings();
  if (!recs.length) return console.log("No recordings found.");

  const pick = await select({
    title: "\nWhich recording?",
    options: recs.map((rec: any) => recordingLabel({ rec })),
  });
  await exportRecording({ id: recs[pick].id });
  reindex();
};

const exportAll = async () => {
  const recs = await listRecordings();
  console.log("");
  for (const rec of recs) {
    await exportRecording({ id: rec.id });
  }
  console.log(`✓ Exported ${recs.length} recordings to ${EXPORT_ROOT}`);
  reindex();
};

const MENU = [
  { name: "Search conversations", run: searchInteractive },
  { name: "List recordings", run: list },
  { name: "Export one recording", run: exportOne },
  { name: "Export all recordings", run: exportAll },
  { name: "Rebuild search index", run: async () => reindex() },
];

const main = async () => {
  const { args, opts } = cli;
  const [cmd, arg] = args;

  // Search needs no API key — it only reads what was already exported.
  const query = opts["--search"] || (cmd === "search" ? args.slice(1).join(" ") : "");
  if (query) return search({ query });
  if (cmd === "index") return reindex();

  if (!appKey()) {
    return console.log(
      "No API key found.\nSet POCKET_APP_KEY or put the key in ~/.config/pocket/key"
    );
  }

  if (cmd === "list") return list();
  if (cmd === "export" && arg === "all") return exportAll();
  if (cmd === "export" && arg) {
    await exportRecording({ id: arg });
    return reindex();
  }
  if (cmd === "export") return exportOne();

  const pick = await select({
    title: "Pocket",
    options: MENU.map((item) => item.name),
  });
  await MENU[pick].run();
};

main();
