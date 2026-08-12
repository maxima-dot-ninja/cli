import * as readline from "readline";

type Keypress = { char: string; key: any };

// One permanent keypress listener feeding a queue — keys typed while a
// fetch is in flight (or piped in one chunk) are held, never dropped.
const pending: Keypress[] = [];
let waiting: ((keypress: Keypress) => void) | null = null;
let initialized = false;

const init = () => {
  if (initialized) return;
  initialized = true;
  readline.emitKeypressEvents(process.stdin);
  if (process.stdin.isTTY) process.stdin.setRawMode(true);
  process.on("exit", () => {
    if (process.stdin.isTTY) process.stdin.setRawMode(false);
  });
  process.stdin.on("keypress", (char, key) => {
    if (key?.ctrl && key?.name === "c") process.exit(0);
    if (!waiting) return void pending.push({ char, key });
    const resolve = waiting;
    waiting = null;
    resolve({ char, key });
  });
};

const nextKey = () => {
  init();
  process.stdin.resume();
  const held = pending.shift();
  if (held) return Promise.resolve(held);
  return new Promise<Keypress>((resolve) => {
    waiting = resolve;
  });
};

const draw = ({
  options,
  index,
  first,
}: {
  options: string[];
  index: number;
  first: boolean;
}) => {
  if (!first) process.stdout.write(`\x1B[${options.length}A`);
  for (const [i, option] of options.entries()) {
    const marker = i === index ? "❯" : " ";
    process.stdout.write(`\x1B[2K  ${marker} ${option}\n`);
  }
};

// Free-text input on the same raw-mode listener the menu uses — readline's own
// question() would fight it for stdin.
export const prompt = async ({ title }: { title: string }) => {
  process.stdout.write(`${title} `);
  let text = "";

  while (true) {
    const { char, key = {} } = await nextKey();
    if (key?.name === "return" || key?.name === "enter") break;
    if (key?.name === "backspace") {
      if (text) process.stdout.write("\b \b");
      text = text.slice(0, -1);
      continue;
    }
    if (!char || key?.ctrl || key?.meta) continue;
    text += char;
    process.stdout.write(char);
  }

  process.stdin.pause();
  console.log("");
  return text.trim();
};

// Arrow-key menu: ↑/↓ to move, digits to jump, enter to confirm.
export const select = async ({
  title,
  options,
}: {
  title: string;
  options: string[];
}) => {
  console.log(`${title}\n`);
  let index = 0;
  draw({ options, index, first: true });

  while (true) {
    const { char, key = {} } = await nextKey();
    if (key?.name === "return" || key?.name === "enter") break;

    const digit = parseInt(char);
    if (digit >= 1 && digit <= options.length) index = digit - 1;
    if (key?.name === "up") index = (index - 1 + options.length) % options.length;
    if (key?.name === "down") index = (index + 1) % options.length;
    draw({ options, index, first: false });
  }

  process.stdin.pause();
  console.log("");
  return index;
};
