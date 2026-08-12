import * as os from "os";
import * as path from "path";

// One fixed home for exports, never process.cwd() — the search index and any
// agent reading these files need a path that does not depend on where pocket ran.
export const EXPORT_ROOT =
  process.env.POCKET_EXPORT_DIR || path.join(os.homedir(), "dev", "pocket-exports");
