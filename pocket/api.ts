import * as fs from "fs";
import * as os from "os";
import * as path from "path";

const BASE_URL = "https://public.heypocketai.com/api/v1";
const KEY_FILE = path.join(os.homedir(), ".config", "pocket", "key");

export const appKey = () => {
  if (process.env.POCKET_APP_KEY) return process.env.POCKET_APP_KEY;
  if (!fs.existsSync(KEY_FILE)) return "";
  return fs.readFileSync(KEY_FILE, "utf8").trim();
};

const request = async ({ endpoint }: { endpoint: string }) => {
  console.log(`→ GET ${endpoint}`);
  const res = await fetch(`${BASE_URL}${endpoint}`, {
    headers: { Authorization: `Bearer ${appKey()}` },
  });
  if (!res.ok) {
    console.log(`✗ ${res.status} — ${res.statusText}`);
    return null;
  }

  const body = await res.json();
  if (body?.success === false) {
    console.log(`✗ ${body.error}`);
    return null;
  }

  return body.data;
};

export const listRecordings = async () => {
  const data = await request({ endpoint: "/public/recordings?limit=100" });
  return data || [];
};

export const getRecording = ({ id }: { id: string }) =>
  request({
    endpoint: `/public/recordings/${id}?include_transcript=true&include_summarizations=true`,
  });
