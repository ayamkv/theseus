// Note that the specific exe here doesn't matter, we just need the same types found in all of them.
import type * as exe from "./exe/basicdd/basicdd.js";

(self as any).send_to_host = (func: string, args: any[], retAddr: number) => {
  const obj: exe.Msg = { func, args, retAddr };
  self.postMessage(obj);
};

/// Update the boot screen on the page.
function bootStatus(text: string) {
  self.postMessage({ func: "boot_status", args: [text], retAddr: 0 });
}

async function run(start: StartMessage) {
  const { name, memory, files = [], cwd, saves } = start;
  // Fetch the game data alongside the wasm download and compile.
  const data = files.map(async (file) => {
    const response = await fetch(new URL(`.${cwd}/${file}`, self.location.origin + "/"));
    if (!response.ok) throw new Error(`could not fetch ${file}`);
    return [`${cwd}/${file}`, new Uint8Array(await response.arrayBuffer())] as const;
  });
  // The wasm load may fail first; keep an early data failure from being
  // reported as unhandled before it is awaited below.
  for (const pending of data) pending.catch(() => {});
  bootStatus("Loading engine…");
  const exe = await import(`./exe/${name}/${name}.js`);
  await exe.default(/* module */ undefined, memory);
  bootStatus("Loading game data…");
  for (const [path, bytes] of await Promise.all(data)) {
    exe.mount_file(path, bytes);
  }
  // Saved files win over the shipped ones.
  for (const [path, data] of saves) exe.mount_file(path, data);
  if (cwd) exe.set_current_dir(cwd);
  bootStatus("Starting…");
  exe.main();
}

export interface StartMessage {
  name: string;
  memory: WebAssembly.Memory;
  files?: string[];
  cwd?: string;
  saves: [string, Uint8Array][];
}

self.onmessage = (e: MessageEvent<StartMessage>) => {
  run(e.data).catch((e) => {
    console.error(e);
    self.postMessage({
      func: "boot_error",
      args: [String((e as Error)?.message ?? e)],
      retAddr: 0,
    });
  });
};
