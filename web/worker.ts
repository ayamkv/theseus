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
  bootStatus("Loading engine…");
  const exe = await import(`./exe/${name}/${name}.js`);
  await exe.default(/* module */ undefined, memory);
  bootStatus("Loading game data…");
  for (const file of files) {
    const response = await fetch(new URL(`.${cwd}/${file}`, self.location.origin + "/"));
    if (!response.ok) throw new Error(`could not fetch ${file}`);
    exe.mount_file(`${cwd}/${file}`, new Uint8Array(await response.arrayBuffer()));
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
