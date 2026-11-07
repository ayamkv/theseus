// Note that the specific exe here doesn't matter, we just need the same types found in all of them.
import type * as exe from "./exe/basicdd/basicdd.js";
import type * as worker from "./worker.js";

/// KeyboardEvent.code to [PC scan code, VK_* code, extended]. See the SDL host's key table.
const KEY_MAP: Record<string, [number, number, number]> = (() => {
  const map: Record<string, [number, number, number]> = {
    Escape: [0x01, 0x1b, 0],
    Minus: [0x0c, 0xbd, 0],
    Equal: [0x0d, 0xbb, 0],
    Backspace: [0x0e, 0x08, 0],
    Tab: [0x0f, 0x09, 0],
    BracketLeft: [0x1a, 0xdb, 0],
    BracketRight: [0x1b, 0xdd, 0],
    Enter: [0x1c, 0x0d, 0],
    ControlLeft: [0x1d, 0xa2, 0],
    Semicolon: [0x27, 0xba, 0],
    Quote: [0x28, 0xde, 0],
    Backquote: [0x29, 0xc0, 0],
    ShiftLeft: [0x2a, 0xa0, 0],
    Backslash: [0x2b, 0xdc, 0],
    Comma: [0x33, 0xbc, 0],
    Period: [0x34, 0xbe, 0],
    Slash: [0x35, 0xbf, 0],
    ShiftRight: [0x36, 0xa1, 0],
    AltLeft: [0x38, 0xa4, 0],
    Space: [0x39, 0x20, 0],
    CapsLock: [0x3a, 0x14, 0],
    Home: [0x47, 0x24, 1],
    ArrowUp: [0x48, 0x26, 1],
    PageUp: [0x49, 0x21, 1],
    ArrowLeft: [0x4b, 0x25, 1],
    ArrowRight: [0x4d, 0x27, 1],
    End: [0x4f, 0x23, 1],
    ArrowDown: [0x50, 0x28, 1],
    PageDown: [0x51, 0x22, 1],
    Insert: [0x52, 0x2d, 1],
    Delete: [0x53, 0x2e, 1],
  };
  const rows: [string, number][] = [["QWERTYUIOP", 0x10], ["ASDFGHJKL", 0x1e], ["ZXCVBNM", 0x2c]];
  for (const [letters, first] of rows) {
    [...letters].forEach((c, i) => (map[`Key${c}`] = [first + i, c.charCodeAt(0), 0]));
  }
  [..."1234567890"].forEach((c, i) => (map[`Digit${c}`] = [0x02 + i, c.charCodeAt(0), 0]));
  return map;
})();

/// Characters a phone keyboard can produce, as the browser `code` of the key
/// that types them and whether shift is held. Mirrors the guest's
/// `vkey_to_char` in win32/winapi/src/user32/input.rs.
const CHAR_KEYS: Record<string, [code: string, shift: boolean]> = (() => {
  const map: Record<string, [string, boolean]> = {};
  for (let i = 0; i < 26; i++) {
    const lower = String.fromCharCode(97 + i);
    const upper = lower.toUpperCase();
    map[lower] = [`Key${upper}`, false];
    map[upper] = [`Key${upper}`, true];
  }
  for (const digit of "1234567890") map[digit] = [`Digit${digit}`, false];
  for (const [plain, shifted, code] of [
    ["`", "~", "Backquote"],
    ["-", "_", "Minus"],
    ["=", "+", "Equal"],
    ["[", "{", "BracketLeft"],
    ["]", "}", "BracketRight"],
    ["\\", "|", "Backslash"],
    [";", ":", "Semicolon"],
    ["'", '"', "Quote"],
    [",", "<", "Comma"],
    [".", ">", "Period"],
    ["/", "?", "Slash"],
    ["1", "!", "Digit1"],
    ["2", "@", "Digit2"],
    ["3", "#", "Digit3"],
    ["4", "$", "Digit4"],
    ["5", "%", "Digit5"],
    ["6", "^", "Digit6"],
    ["7", "&", "Digit7"],
    ["8", "*", "Digit8"],
    ["9", "(", "Digit9"],
    ["0", ")", "Digit0"],
  ] as const) {
    map[plain] = [code, false];
    map[shifted] = [code, true];
  }
  map[" "] = ["Space", false];
  map["\n"] = ["Enter", false];
  map["\r"] = ["Enter", false];
  return map;
})();

class MessageQueue {
  private messages: Event[] = [];
  private waiter: ((value: Event) => void) | undefined;

  poll(): Event | undefined {
    return this.messages.shift();
  }

  /// Enqueue a key press as if it came from the page's keyboard.
  sendKey(type: "keydown" | "keyup", code: string) {
    this.enqueue(
      new KeyboardEvent(type, { code, key: code, bubbles: true, cancelable: true }),
    );
  }

  wait(): Promise<Event> {
    const msg = this.poll();
    if (msg !== undefined) {
      return Promise.resolve(msg);
    }
    const { promise, resolve } = Promise.withResolvers<Event>();
    this.waiter = resolve;
    return promise;
  }

  private enqueue = (e: Event) => {
    e.preventDefault();
    if (this.waiter) {
      this.waiter(e);
      this.waiter = undefined;
    } else {
      this.messages.push(e);
    }
  };
  private discard = (e: Event) => {
    e.preventDefault();
  };

  listen(dom: HTMLCanvasElement) {
    dom.onmousedown = this.enqueue;
    dom.onmouseup = this.enqueue;
    dom.onmousemove = this.enqueue;
    dom.oncontextmenu = this.discard;
    const onKey = (e: KeyboardEvent) => {
      // Leave browser shortcuts alone.
      if (KEY_MAP[e.code] && !e.ctrlKey && !e.metaKey) this.enqueue(e);
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("keyup", onKey);
  }
}

interface Surface {
  canvas: HTMLCanvasElement;
  ctx: CanvasRenderingContext2D;
  /// Reused across frames so set_pixels does not allocate.
  imageData: ImageData;
}

class Host implements exe.WasmHost {
  consoleDom = document.createElement("pre");
  consoleOutputDecoder = new TextDecoder();
  consoleOutput = "";
  window_: HTMLCanvasElement | undefined;

  surfaces: Map<number, Surface> = new Map();
  nextSurface = 1;
  messageQueue = new MessageQueue();
  windowCtx: CanvasRenderingContext2D | undefined;

  audio: AudioContext | undefined;
  /// AudioContext time at which scheduled audio runs out.
  audioEnd = 0;
  /// While audio is blocked, performance.now() at which discarded audio would have ended.
  silentEnd = 0;

  /// Key presses waiting to be handed to the guest, one at a time.
  private pendingKeys: { code: string; shift: boolean }[] = [];
  private sendingKey = false;

  constructor(public wasmMemory: WebAssembly.Memory, public savePrefix: string) {
    this.consoleDom.id = "console";
    document.body.appendChild(this.consoleDom);
    // Browsers only allow audio to start from a user gesture.
    const unlock = () => {
      this.audio ??= new AudioContext();
      this.audio.resume();
    };
    window.addEventListener("pointerdown", unlock);
    window.addEventListener("keydown", unlock);
    this.createMobileKeyboard();
  }

  /// Type text from the on-screen keyboard, as if it came from a real one.
  typeText(text: string) {
    for (const ch of text) {
      const key = CHAR_KEYS[ch];
      if (key) this.queueKey(key[0], key[1]);
    }
  }

  /// Press a non-text key such as Backspace or Enter.
  pressKey(code: string) {
    this.queueKey(code, false);
  }

  private queueKey(code: string, shift: boolean) {
    this.pendingKeys.push({ code, shift });
    this.sendNextKey();
  }

  private sendNextKey() {
    if (this.sendingKey) return;
    const key = this.pendingKeys.shift();
    if (!key) return;
    this.sendingKey = true;
    // Deliver the press and release separately, so the guest samples the key
    // as down on at least one pump. Holding shift until the release keeps the
    // guest's TranslateMessage seeing it when it turns the key into a WM_CHAR.
    if (key.shift) this.messageQueue.sendKey("keydown", "ShiftLeft");
    this.messageQueue.sendKey("keydown", key.code);
    setTimeout(() => {
      this.messageQueue.sendKey("keyup", key.code);
      if (key.shift) this.messageQueue.sendKey("keyup", "ShiftLeft");
      this.sendingKey = false;
      this.sendNextKey();
    }, 40);
  }

  /// A phone has no keyboard, so offer a button that opens the browser's
  /// native text prompt and feeds the result to the guest as keystrokes.
  private createMobileKeyboard() {
    const toggle = document.createElement("button");
    toggle.className = "mobile-keyboard-toggle";
    toggle.title = "Type text";
    toggle.setAttribute("aria-label", "Type text");
    // Drawn rather than an emoji so it looks the same on every platform.
    toggle.innerHTML = `<svg viewBox="0 0 16 12" width="32" height="24" shape-rendering="crispEdges" aria-hidden="true">
      <path fill="#004918" d="M1 0h14v1h1v10h-1v1H1v-1H0V1h1z"/>
      <path fill="currentColor" d="M2 2h2v2H2zM5 2h2v2H5zM8 2h2v2H8zM11 2h3v2h-3zM2 5h3v2H2zM6 5h2v2H6zM9 5h2v2H9zM12 5h2v2h-2zM4 8h8v2H4z"/>
    </svg>`;
    toggle.addEventListener("click", () => {
      // Using the native prompt avoids building an input UI; the returned
      // text is typed into the game, then Enter confirms it.
      const text = window.prompt("Type text for the game:");
      if (text === null) return;
      this.typeText(text);
      this.pressKey("Enter");
    });
    document.body.appendChild(toggle);
  }

  /// Called by the worker as loading progresses, for the boot screen.
  boot_status(text: string) {
    const el = document.querySelector("#boot .boot-text");
    if (el) el.textContent = text;
  }

  /// Show a failure instead of leaving the loading screen up forever.
  boot_error(text: string) {
    const boot = document.getElementById("boot");
    if (!boot) return;
    boot.classList.add("error");
    const el = boot.querySelector(".boot-text");
    if (el) el.textContent = `Could not start: ${text}`;
  }

  private bootFinished = false;

  /// Drop the loading screen once the game has drawn its first frame.
  private finishBoot() {
    if (this.bootFinished) return;
    this.bootFinished = true;
    const boot = document.getElementById("boot");
    if (boot) {
      requestAnimationFrame(() => {
        boot.classList.add("done");
        setTimeout(() => boot.remove(), 300);
      });
    }
    this.registerServiceWorker();
  }

  /// Keep the page available offline and make it installable.
  private registerServiceWorker() {
    if (!(import.meta as any).env?.PROD) return;
    navigator.serviceWorker
      ?.register("./sw.js")
      .then(() => navigator.storage?.persist?.())
      .catch((e) => console.warn("service worker", e));
  }

  onMessage(e: MessageEvent<exe.Msg>) {
    const msg = e.data;
    const ret = (this as any)[msg.func](...msg.args);
    if (msg.retAddr) {
      if (ret instanceof Promise) {
        ret.then((ret) => this.finishSync(msg.retAddr, ret));
        return;
      }
      this.finishSync(msg.retAddr, ret);
    }
  }

  finishSync(retAddr: number, ret: number | number[]): void {
    const arr = Array.isArray(ret) ? ret : [ret];
    if (!Number.isFinite(arr[0]) || arr[0] == 0) {
      // For synchronization to work, we must put a non-zero value in the first slot.
      // If this hits we messed up the sync/non-sync ness of some API.
      throw new Error();
    }
    const ints = new Int32Array(this.wasmMemory.buffer, retAddr, arr.length);
    ints.set(arr);
    Atomics.notify(ints, 0, 1);
  }

  console_write(ptr: number, len: number): void {
    // We must copy from wasm memory to decode.
    const inBuf = new Uint8Array(this.wasmMemory.buffer, ptr, len).slice();
    this.consoleOutput += this.consoleOutputDecoder.decode(inBuf, { stream: true });
    this.consoleDom.innerText = this.consoleOutput;
  }

  create_surface(width: number, height: number): number {
    const canvas = document.createElement("canvas");
    canvas.width = width;
    canvas.height = height;
    canvas.style.display = "none";
    document.body.appendChild(canvas);

    const id = this.nextSurface++;
    // Keep one ImageData per surface: allocating it every frame is a lot of
    // garbage for a 640x480 surface.
    this.surfaces.set(id, {
      canvas,
      ctx: canvas.getContext("2d")!,
      imageData: new ImageData(width, height),
    });
    return id;
  }

  create_window(_title: string, width: number, height: number): number {
    this.window_ = document.createElement("canvas");
    this.window_.className = "window";
    this.window_.width = width;
    this.window_.height = height;
    document.body.appendChild(this.window_);
    this.windowCtx = this.window_.getContext("2d")!;
    this.messageQueue.listen(this.window_);
    return 1;
  }

  resize_window(_id: number, width: number, height: number): void {
    this.window_!.width = width;
    this.window_!.height = height;
  }

  render(_window_id: number, surface_id: number) {
    const surface = this.surfaces.get(surface_id)!;
    this.windowCtx!.drawImage(surface.canvas, 0, 0);
    this.finishBoot();
  }

  set_pixels(id: number, ptr: number, len: number): number {
    // TODO: investigate using OffscreenCanvas here instead,
    // https://news.ycombinator.com/item?id=48297805
    const surface = this.surfaces.get(id)!;
    surface.imageData.data.set(
      new Uint8ClampedArray(this.wasmMemory.buffer, ptr, len),
    );
    surface.ctx.putImageData(surface.imageData, 0, 0);
    return 1;
  }

  /// The guest works in canvas pixels, but the canvas may be scaled to fit the
  /// screen, so translate from screen coordinates.
  private canvasCoords(e: MouseEvent): [number, number] {
    const canvas = this.window_;
    const rect = canvas?.getBoundingClientRect();
    if (!canvas || !rect || rect.width === 0 || rect.height === 0) {
      return [e.offsetX, e.offsetY];
    }
    return [
      Math.round(((e.clientX - rect.left) * canvas.width) / rect.width),
      Math.round(((e.clientY - rect.top) * canvas.height) / rect.height),
    ];
  }

  private serializeMessage(event: Event): number[] {
    // see wasm.rs:parse_message
    const typeToCode: Record<string, number> = {
      mousedown: 2,
      mouseup: 3,
      mousemove: 4,
      keydown: 5,
      keyup: 6,
    };
    const code = typeToCode[event.type];
    if (code === undefined) throw new Error();
    switch (event.type) {
      case "mousedown":
      case "mouseup":
      case "mousemove": {
        const e = event as MouseEvent;
        // MouseEvent.buttons has right and middle swapped relative to button numbers.
        const b = e.buttons;
        const held = (b & 1) | ((b & 2) << 1) | ((b & 4) >> 1);
        const [x, y] = this.canvasCoords(e);
        return [typeToCode[e.type]!, x, y, (1 << e.button) | (held << 16)];
      }
      case "keydown":
      case "keyup": {
        const e = event as KeyboardEvent;
        const [scancode, vkey, extended] = KEY_MAP[e.code]!;
        return [typeToCode[e.type]!, scancode, vkey, extended | (e.repeat ? 2 : 0)];
      }
      default:
        throw new Error();
    }
  }

  poll_message(): number[] {
    const event = this.messageQueue.poll();
    return event ? this.serializeMessage(event) : [-1];
  }

  async wait_message(): Promise<number[]> {
    const event = await this.messageQueue.wait();
    return this.serializeMessage(event);
  }

  write_file(path: string, ptr: number, len: number): number {
    const bytes = new Uint8Array(this.wasmMemory.buffer, ptr, len).slice();
    let text = "";
    for (const b of bytes) text += String.fromCharCode(b);
    try {
      localStorage.setItem(this.savePrefix + path, btoa(text));
    } catch (e) {
      console.warn("could not save", path, e);
    }
    return 1;
  }

  /// Returns the milliseconds of audio queued, plus one: a sync reply cannot be zero.
  play_audio(rate: number, channels: number, ptr: number, len: number): number {
    const frames = len / 2 / channels;
    const seconds = frames / rate;
    const ctx = this.audio;
    if (!ctx || ctx.state !== "running") {
      // Keep the guest's mixer running in real time until audio is allowed.
      const now = performance.now();
      this.silentEnd = Math.max(this.silentEnd, now) + seconds * 1000;
      return Math.ceil(this.silentEnd - now) + 1;
    }
    // The source pointer may be unaligned, and may not be viewed as shared memory by Web Audio.
    const bytes = new Uint8Array(this.wasmMemory.buffer, ptr, len).slice();
    const samples = new Int16Array(bytes.buffer);
    const buffer = ctx.createBuffer(channels, frames, rate);
    for (let c = 0; c < channels; c++) {
      const out = buffer.getChannelData(c);
      for (let i = 0; i < frames; i++) out[i] = samples[i * channels + c]! / 32768;
    }
    const source = ctx.createBufferSource();
    source.buffer = buffer;
    source.connect(ctx.destination);
    const start = Math.max(this.audioEnd, ctx.currentTime + 0.02);
    source.start(start);
    this.audioEnd = start + seconds;
    return Math.ceil((this.audioEnd - ctx.currentTime) * 1000) + 1;
  }
}

/// Programs to run, chosen with `?exe=`.
// TODO: we'll put program params in the values of this map.
const PROGRAMS = new Map<string, { files?: string[]; cwd?: string }>([
  ["mine", {}],
  ["basicdd", {}],
  ["winapi", {}],
  ["lemonade", { files: ["Lemonade.RB", "Lemonade.dat"], cwd: "/game" }],
]);

function loadSaves(prefix: string): [string, Uint8Array][] {
  const saves: [string, Uint8Array][] = [];
  for (let i = 0; i < localStorage.length; i++) {
    const key = localStorage.key(i)!;
    if (key.startsWith(prefix)) {
      const text = atob(localStorage.getItem(key)!);
      saves.push([key.slice(prefix.length), Uint8Array.from(text, (c) => c.charCodeAt(0))]);
    }
  }
  return saves;
}

/// When the server does not send the cross-origin isolation headers, the
/// service worker can add them. Register it and reload once so the document
/// itself is isolated.
async function ensureCrossOriginIsolated() {
  const flag = "lemonade:reload-for-isolation";
  if (window.crossOriginIsolated) {
    sessionStorage.removeItem(flag);
    return;
  }
  const sw = navigator.serviceWorker;
  if (!(import.meta as any).env?.PROD || !sw || !window.isSecureContext) return;
  if (sessionStorage.getItem(flag)) return;
  sessionStorage.setItem(flag, "1");
  try {
    await sw.register("./sw.js");
    await sw.ready;
    location.reload();
  } catch (e) {
    console.warn("service worker", e);
  }
}

async function main() {
  await ensureCrossOriginIsolated();
  if (!window.SharedArrayBuffer) {
    document.body.innerText = "SharedArrayBuffer is not supported; possibly try reloading";
    return;
  }

  const wasmPageSize = 64 << 10;
  const memory = new WebAssembly.Memory({
    // memory args are in units of wasm page size
    initial: (16 << 20) / wasmPageSize,
    maximum: (512 << 20) / wasmPageSize,
    shared: true,
  });

  const params = new URLSearchParams(window.location.search);
  const name = params.get("exe") ?? "mine";
  const savePrefix = `theseus:${name}:`;

  const host = new Host(memory, savePrefix);
  (window as any).theseusHost = host;
  const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
  worker.onmessage = (e) => host.onMessage(e);

  const program = PROGRAMS.get(name);
  if (!program) {
    document.body.innerText = `no such program ${name}`;
    return;
  }

  const message: worker.StartMessage = {
    ...program,
    name,
    memory,
    saves: loadSaves(savePrefix),
  };
  worker.postMessage(message);
}
main().catch((e) => console.error(e));
