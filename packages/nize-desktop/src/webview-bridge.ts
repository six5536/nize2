// @zen-component: PLAN-015-BridgeClient
//
// WebView Bridge client — injected into the Tauri webview (dev builds only).
//
// Opens a WebSocket to the bridge MCP server, listens for JSON commands,
// executes against the live DOM (parent + iframe), and returns results.

const BRIDGE_PORT = 19570;
const CONSOLE_BUFFER_SIZE = 100;

// --- Types ---

interface BridgeCommand {
  id: string;
  method: string;
  params?: Record<string, unknown>;
}

interface BridgeResponse {
  id: string;
  result?: unknown;
  error?: string;
}

interface ConsoleEntry {
  level: string;
  message: string;
  timestamp: number;
}

interface SnapshotNode {
  role: string;
  name: string;
  tag: string;
  ref?: number;
  value?: string;
  children?: SnapshotNode[];
}

// --- State ---

const consoleBuffer: ConsoleEntry[] = [];
let refCounter = 0;
let refMap: Map<number, Element> = new Map();

// --- Console interceptor ---

// @zen-impl: PLAN-015-1.3
const originalConsole = {
  log: console.log,
  warn: console.warn,
  error: console.error,
  info: console.info,
  debug: console.debug,
};

function interceptConsole() {
  for (const level of ["log", "warn", "error", "info", "debug"] as const) {
    const original = originalConsole[level];
    console[level] = (...args: unknown[]) => {
      consoleBuffer.push({
        level,
        message: args.map((a) => (typeof a === "string" ? a : JSON.stringify(a))).join(" "),
        timestamp: Date.now(),
      });
      if (consoleBuffer.length > CONSOLE_BUFFER_SIZE) {
        consoleBuffer.shift();
      }
      original.apply(console, args);
    };
  }
}

// --- DOM snapshot ---

// @zen-impl: PLAN-015-1.2
function computeRole(el: Element): string {
  const explicit = el.getAttribute("role");
  if (explicit) return explicit;

  const tag = el.tagName.toLowerCase();
  const roleMap: Record<string, string> = {
    a: "link",
    button: "button",
    input: "textbox",
    textarea: "textbox",
    select: "combobox",
    option: "option",
    img: "img",
    nav: "navigation",
    main: "main",
    header: "banner",
    footer: "contentinfo",
    aside: "complementary",
    section: "region",
    form: "form",
    table: "table",
    ul: "list",
    ol: "list",
    li: "listitem",
    h1: "heading",
    h2: "heading",
    h3: "heading",
    h4: "heading",
    h5: "heading",
    h6: "heading",
    dialog: "dialog",
  };
  return roleMap[tag] || "generic";
}

function computeName(el: Element): string {
  const ariaLabel = el.getAttribute("aria-label");
  if (ariaLabel) return ariaLabel;

  const labelledBy = el.getAttribute("aria-labelledby");
  if (labelledBy) {
    const labelEl = el.ownerDocument.getElementById(labelledBy);
    if (labelEl) return labelEl.textContent?.trim() || "";
  }

  if (el.tagName === "IMG") return (el as HTMLImageElement).alt || "";
  if (el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.tagName === "SELECT") {
    const id = el.getAttribute("id");
    if (id) {
      const label = el.ownerDocument.querySelector(`label[for="${id}"]`);
      if (label) return label.textContent?.trim() || "";
    }
    return el.getAttribute("placeholder") || el.getAttribute("name") || "";
  }

  // For leaf nodes, use direct text content (not children's text)
  if (el.children.length === 0) {
    return el.textContent?.trim().slice(0, 100) || "";
  }

  return "";
}

function computeValue(el: Element): string | undefined {
  if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
    return el.value;
  }
  if (el instanceof HTMLSelectElement) {
    return el.options[el.selectedIndex]?.text;
  }
  return undefined;
}

function isInteractive(el: Element): boolean {
  const tag = el.tagName.toLowerCase();
  if (["a", "button", "input", "textarea", "select", "details", "summary"].includes(tag)) {
    return true;
  }
  if (el.getAttribute("tabindex") !== null) return true;
  if (el.getAttribute("onclick") !== null) return true;
  if (el.getAttribute("role") && ["button", "link", "textbox", "checkbox", "radio", "switch", "tab", "menuitem"].includes(el.getAttribute("role")!)) {
    return true;
  }
  return false;
}

function isVisible(el: Element): boolean {
  if (el instanceof HTMLElement) {
    if (el.offsetWidth === 0 && el.offsetHeight === 0) return false;
    const style = getComputedStyle(el);
    if (style.display === "none" || style.visibility === "hidden") return false;
  }
  return true;
}

function snapshotElement(el: Element, doc: Document): SnapshotNode | null {
  if (!isVisible(el)) return null;

  const tag = el.tagName.toLowerCase();
  // Skip script, style, svg internals, and the bridge script itself
  if (["script", "style", "noscript", "link", "meta"].includes(tag)) return null;

  const role = computeRole(el);
  const name = computeName(el);
  const value = computeValue(el);

  let ref: number | undefined;
  if (isInteractive(el)) {
    ref = refCounter++;
    refMap.set(ref, el);
  }

  const children: SnapshotNode[] = [];

  // Traverse into same-origin iframes
  if (tag === "iframe") {
    try {
      const iframeDoc = (el as HTMLIFrameElement).contentDocument;
      if (iframeDoc && iframeDoc.body) {
        const iframeChildren = snapshotChildren(iframeDoc.body, iframeDoc);
        children.push(...iframeChildren);
      }
    } catch {
      // Cross-origin — skip
    }
  } else {
    children.push(...snapshotChildren(el, doc));
  }

  // Prune empty generic nodes with no name/value/ref
  if (role === "generic" && !name && value === undefined && ref === undefined && children.length === 0) {
    return null;
  }

  // Flatten generic wrappers with a single child and no meaningful attributes
  if (role === "generic" && !name && value === undefined && ref === undefined && children.length === 1) {
    return children[0];
  }

  const node: SnapshotNode = { role, name, tag };
  if (ref !== undefined) node.ref = ref;
  if (value !== undefined) node.value = value;
  if (children.length > 0) node.children = children;

  return node;
}

function snapshotChildren(el: Element, doc: Document): SnapshotNode[] {
  const results: SnapshotNode[] = [];
  for (const child of el.children) {
    const node = snapshotElement(child, doc);
    if (node) results.push(node);
  }
  return results;
}

function takeSnapshot(): SnapshotNode | null {
  refCounter = 0;
  refMap = new Map();
  return snapshotElement(document.documentElement, document);
}

// --- Snapshot YAML formatter ---

function snapshotToYaml(node: SnapshotNode | null, indent: number = 0): string {
  if (!node) return "(empty)\n";

  const pad = "  ".repeat(indent);
  let line = `${pad}- ${node.role}`;
  if (node.name) line += ` "${node.name}"`;
  if (node.ref !== undefined) line += ` [ref=${node.ref}]`;
  if (node.value !== undefined) line += ` value="${node.value}"`;
  line += "\n";

  if (node.children) {
    for (const child of node.children) {
      line += snapshotToYaml(child, indent + 1);
    }
  }

  return line;
}

// --- Command dispatcher ---

function findElement(params: Record<string, unknown>): Element | null {
  if (typeof params.ref === "number") {
    return refMap.get(params.ref) || null;
  }
  if (typeof params.selector === "string") {
    return document.querySelector(params.selector) || findInIframes(params.selector);
  }
  if (typeof params.text === "string") {
    return findByText(params.text);
  }
  return null;
}

function findInIframes(selector: string): Element | null {
  const iframes = document.querySelectorAll("iframe");
  for (const iframe of iframes) {
    try {
      const doc = iframe.contentDocument;
      if (doc) {
        const el = doc.querySelector(selector);
        if (el) return el;
      }
    } catch {
      // Cross-origin
    }
  }
  return null;
}

function findByText(text: string): Element | null {
  const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_ELEMENT);
  let node: Node | null = walker.currentNode;
  while (node) {
    if (node instanceof Element && node.textContent?.trim() === text) {
      return node;
    }
    node = walker.nextNode();
  }
  // Also search in iframes
  const iframes = document.querySelectorAll("iframe");
  for (const iframe of iframes) {
    try {
      const doc = iframe.contentDocument;
      if (doc && doc.body) {
        const iframeWalker = doc.createTreeWalker(doc.body, NodeFilter.SHOW_ELEMENT);
        let iNode: Node | null = iframeWalker.currentNode;
        while (iNode) {
          if (iNode instanceof Element && iNode.textContent?.trim() === text) {
            return iNode;
          }
          iNode = iframeWalker.nextNode();
        }
      }
    } catch {
      // Cross-origin
    }
  }
  return null;
}

// @zen-impl: PLAN-015-1.1
async function handleCommand(cmd: BridgeCommand): Promise<BridgeResponse> {
  try {
    switch (cmd.method) {
      case "snapshot": {
        const tree = takeSnapshot();
        return { id: cmd.id, result: snapshotToYaml(tree) };
      }

      case "evaluate": {
        const expr = cmd.params?.expression as string;
        if (!expr) return { id: cmd.id, error: "missing 'expression' param" };
        // eslint-disable-next-line no-eval
        const result = eval(expr);
        return { id: cmd.id, result: JSON.parse(JSON.stringify(result ?? null)) };
      }

      case "click": {
        const el = findElement(cmd.params || {});
        if (!el) return { id: cmd.id, error: "element not found" };
        if (el instanceof HTMLElement) {
          el.focus();
          el.click();
        }
        return { id: cmd.id, result: null };
      }

      case "fill": {
        const el = findElement(cmd.params || {});
        if (!el) return { id: cmd.id, error: "element not found" };
        const value = cmd.params?.value as string;
        if (value === undefined) return { id: cmd.id, error: "missing 'value' param" };
        if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) {
          // Use native setter to properly trigger React state updates
          const nativeSet = Object.getOwnPropertyDescriptor(el instanceof HTMLInputElement ? HTMLInputElement.prototype : HTMLTextAreaElement.prototype, "value")?.set;
          nativeSet?.call(el, value);
          el.dispatchEvent(new Event("input", { bubbles: true }));
          el.dispatchEvent(new Event("change", { bubbles: true }));
        }
        return { id: cmd.id, result: null };
      }

      case "select_option": {
        const el = findElement(cmd.params || {});
        if (!el || !(el instanceof HTMLSelectElement)) {
          return { id: cmd.id, error: "select element not found" };
        }
        const optionValue = cmd.params?.value as string;
        if (optionValue === undefined) return { id: cmd.id, error: "missing 'value' param" };
        el.value = optionValue;
        el.dispatchEvent(new Event("change", { bubbles: true }));
        return { id: cmd.id, result: null };
      }

      case "navigate": {
        const url = cmd.params?.url as string;
        if (!url) return { id: cmd.id, error: "missing 'url' param" };
        const target = (cmd.params?.target as string) || "iframe";
        if (target === "iframe") {
          const iframe = document.querySelector("iframe") as HTMLIFrameElement | null;
          if (iframe) {
            iframe.src = url;
          } else {
            return { id: cmd.id, error: "no iframe found" };
          }
        } else {
          window.location.href = url;
        }
        return { id: cmd.id, result: null };
      }

      case "console": {
        const limit = (cmd.params?.limit as number) || CONSOLE_BUFFER_SIZE;
        return { id: cmd.id, result: consoleBuffer.slice(-limit) };
      }

      case "screenshot": {
        try {
          const { default: html2canvas } = await import("html2canvas");
          const canvas = await html2canvas(document.body, { useCORS: true });
          const dataUrl = canvas.toDataURL("image/png");
          // Strip the data:image/png;base64, prefix
          const base64 = dataUrl.split(",")[1];
          return { id: cmd.id, result: base64 };
        } catch (e) {
          return { id: cmd.id, error: `screenshot failed: ${e}` };
        }
      }

      default:
        return { id: cmd.id, error: `unknown method: ${cmd.method}` };
    }
  } catch (e) {
    return { id: cmd.id, error: String(e) };
  }
}

// --- WebSocket connection ---

let hasConnected = false;

function connect() {
  const ws = new WebSocket(`ws://127.0.0.1:${BRIDGE_PORT}`);

  ws.onopen = () => {
    hasConnected = true;
    originalConsole.log("[webview-bridge] connected to bridge server");
  };

  ws.onmessage = async (event) => {
    try {
      const cmd: BridgeCommand = JSON.parse(event.data as string);
      originalConsole.debug("[webview-bridge] cmd:", cmd.method, JSON.stringify(cmd.params));
      const response = await handleCommand(cmd);
      originalConsole.debug("[webview-bridge] rsp:", JSON.stringify(response).slice(0, 200));
      ws.send(JSON.stringify(response));
    } catch (e) {
      originalConsole.error("[webview-bridge] message handling error:", e);
    }
  };

  ws.onclose = () => {
    if (hasConnected) {
      originalConsole.log("[webview-bridge] disconnected — reconnecting in 2s");
    }
    setTimeout(connect, 2000);
  };

  ws.onerror = () => {
    // onclose will fire after this
  };
}

// --- Bootstrap ---

interceptConsole();
connect();

originalConsole.log("[webview-bridge] bridge client initialized");
