import * as fs from "fs";
import * as path from "path";
import { getRecording } from "./api";
import { folderName, transcriptText, summaryMarkdown } from "./format";
import { EXPORT_ROOT } from "./paths";

export const exportRecording = async ({ id }: { id: string }) => {
  const rec = await getRecording({ id });
  if (!rec) return;

  const dir = path.join(EXPORT_ROOT, folderName({ rec }));
  fs.mkdirSync(dir, { recursive: true });

  const text = transcriptText({ transcript: rec.transcript });
  fs.writeFileSync(path.join(dir, "transcript.txt"), text);
  console.log(`  transcript.txt (${text.length} chars)`);

  const markdown = summaryMarkdown({ rec });
  fs.writeFileSync(path.join(dir, "summary.md"), markdown);
  console.log(`  summary.md (${markdown.length} chars)`);

  if (!text) fs.writeFileSync(path.join(dir, "raw.json"), JSON.stringify(rec, null, 2));

  console.log(`✓ ${dir}\n`);
};
