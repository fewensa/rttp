const ENV_NAMES = [
  "CODEON_OPERATION_MCP_URL",
  "CODEON_OPERATION_CAPABILITY",
  "CODEON_OPERATION_CATALOG_DIGEST",
] as const;
const MAX_RESPONSE_BYTES = 1024 * 1024;
const RETRY_DELAYS_MS = [50, 100, 200, 400, 800] as const;

type JsonObject = Record<string, unknown>;
type ToolResult = {
  content: Array<Record<string, unknown>>;
  details: { content: unknown[]; structuredContent?: unknown; isError: boolean };
};
type PiApi = {
  getAllTools(): Array<{ name: string }>;
  registerTool(tool: {
    name: string;
    label: string;
    description: string;
    parameters: JsonObject;
    execute(
      toolCallId: string,
      parameters: JsonObject,
      signal?: AbortSignal,
    ): Promise<ToolResult>;
  }): void;
  on(
    event: string,
    handler: (
      event: Record<string, unknown>,
      context: { shutdown(): void },
    ) => unknown,
  ): void;
};

function object(value: unknown): value is JsonObject {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function containsBootstrap(value: unknown, values: string[]): boolean {
  if (typeof value === "string") return values.some((secret) => value.includes(secret));
  if (Array.isArray(value)) return value.some((item) => containsBootstrap(item, values));
  return object(value) && Object.values(value).some((item) => containsBootstrap(item, values));
}

function boundedErrorText(value: unknown): string | null {
  if (typeof value !== "string") return null;
  const normalized = value.replace(/[\0\r\n]+/gu, " ").trim();
  return normalized ? normalized.slice(0, 500) : null;
}

function jsonRpcError(error: JsonObject): Error {
  const code = typeof error.code === "number" && Number.isSafeInteger(error.code)
    ? String(error.code)
    : "unknown";
  const message = boundedErrorText(error.message);
  const detail = object(error.data) ? boundedErrorText(error.data.detail) : null;
  return new Error(
    `Codeon operation MCP request failed (${code})${message ? `: ${message}` : ""}${detail ? `; ${detail}` : ""}`,
  );
}

function bootstrap(): { url: string; capability: string; digest: string } {
  const values = ENV_NAMES.map((name) => process.env[name]);
  for (const name of ENV_NAMES) delete process.env[name];

  const [url, capability, digest] = values;
  let parsed: URL;
  try {
    parsed = new URL(url ?? "");
  } catch {
    throw new Error("Codeon operation MCP bootstrap is invalid");
  }
  if (
    !url || url.length > 4096 || !["http:", "https:"].includes(parsed.protocol) ||
    !capability || capability.length > 256 * 1024 || /[\0\r\n]/u.test(capability) ||
    !digest || !/^[0-9a-f]{64}$/u.test(digest)
  ) {
    throw new Error("Codeon operation MCP bootstrap is invalid");
  }
  return { url, capability, digest };
}

async function responseJson(response: Response, allowEmpty = false): Promise<JsonObject> {
  if (!response.ok) throw new Error("Codeon operation MCP request was refused");
  if (!response.body) {
    if (allowEmpty) return {};
    throw new Error("Codeon operation MCP response is invalid");
  }
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let length = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    length += value.byteLength;
    if (length > MAX_RESPONSE_BYTES) {
      await reader.cancel();
      throw new Error("Codeon operation MCP response is too large");
    }
    chunks.push(value);
  }
  if (length === 0 && allowEmpty) return {};
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  try {
    const value: unknown = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
    if (!object(value)) throw new Error();
    return value;
  } catch {
    throw new Error("Codeon operation MCP response is invalid");
  }
}

function sleep(ms: number, signal?: AbortSignal): Promise<void> {
  if (signal?.aborted) return Promise.reject(new Error("Codeon operation MCP request was cancelled"));
  return new Promise((resolve, reject) => {
    const timer = setTimeout(resolve, ms);
    signal?.addEventListener("abort", () => {
      clearTimeout(timer);
      reject(new Error("Codeon operation MCP request was cancelled"));
    }, { once: true });
  });
}

function piContent(content: unknown[]): Array<Record<string, unknown>> {
  return content.map((block) => {
    if (object(block) && block.type === "text" && typeof block.text === "string") {
      return { type: "text", text: block.text };
    }
    if (
      object(block) && block.type === "image" && typeof block.data === "string" &&
      typeof block.mimeType === "string"
    ) {
      return { type: "image", data: block.data, mimeType: block.mimeType };
    }
    return { type: "text", text: JSON.stringify(block) };
  });
}

function isActivationRefusal(result: JsonObject): boolean {
  if (result.isError !== true || !object(result.structuredContent)) return false;
  const refusal = result.structuredContent;
  return Object.keys(refusal).length === 4 &&
    refusal.schema === "codeon.mcp.operation.complete.error/1" &&
    refusal.status === "refused" &&
    refusal.code === "capability_not_active" && refusal.retryable === false;
}

export default async function codeonOperationExtension(pi: PiApi): Promise<void> {
  let url: string | null;
  let capability: string | null;
  let catalogDigest: string | null;
  ({ url, capability, digest: catalogDigest } = bootstrap());
  let requestId = 0;
  let initialized = false;
  const registeredNames = new Set<string>();

  const request = async (
    method: string,
    params: JsonObject | undefined,
    signal?: AbortSignal,
    allowEmpty = false,
  ): Promise<JsonObject> => {
    if (!url || !capability || !catalogDigest) {
      throw new Error("Codeon operation MCP is unavailable");
    }
    const id = params === undefined ? undefined : ++requestId;
    let response: Response;
    try {
      response = await fetch(url, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${capability}`,
          "Content-Type": "application/json",
          Accept: "application/json, text/event-stream",
        },
        body: JSON.stringify({ jsonrpc: "2.0", ...(id === undefined ? {} : { id }), method, ...(params === undefined ? {} : { params }) }),
        signal,
      });
    } catch {
      if (signal?.aborted) throw new Error("Codeon operation MCP request was cancelled");
      throw new Error("Codeon operation MCP request failed");
    }
    const message = await responseJson(response, allowEmpty);
    if (containsBootstrap(message, [url, capability])) {
      throw new Error("Codeon operation MCP response is invalid");
    }
    if (id !== undefined && (message.jsonrpc !== "2.0" || message.id !== id)) {
      throw new Error("Codeon operation MCP response is invalid");
    }
    if (object(message.error)) throw jsonRpcError(message.error);
    if (id === undefined) return message;
    if (!object(message.result)) throw new Error("Codeon operation MCP response is invalid");
    return message.result;
  };

  pi.on("session_start", async (_event, context) => {
    if (initialized) return;
    try {
      const server = await request("initialize", {
        protocolVersion: "2025-06-18",
        capabilities: {},
        clientInfo: { name: "codeon-pi-operation-extension", version: "1" },
      });
      if (server.protocolVersion !== "2025-06-18") {
        throw new Error("Codeon operation MCP response is invalid");
      }
      await request("notifications/initialized", undefined, undefined, true);
      const listed = await request("tools/list", {});
      if (!Array.isArray(listed.tools)) throw new Error("Codeon operation MCP catalog is invalid");

      const occupied = new Set(pi.getAllTools().map((tool) => tool.name));
      const tools = listed.tools.map((value) => {
        if (
          !object(value) || typeof value.name !== "string" || !value.name || value.name.length > 128 ||
          typeof value.description !== "string" || value.description.length > 16 * 1024 ||
          !object(value.inputSchema) || occupied.has(value.name) || registeredNames.has(value.name)
        ) {
          throw new Error("Codeon operation MCP catalog is invalid or collides with a Pi tool");
        }
        registeredNames.add(value.name);
        return { name: value.name, description: value.description, inputSchema: value.inputSchema };
      });

      for (const tool of tools) {
        pi.registerTool({
          name: tool.name,
          label: tool.name,
          description: tool.description,
          parameters: tool.inputSchema,
          async execute(_toolCallId, parameters, signal) {
            let result = await request("tools/call", { name: tool.name, arguments: parameters }, signal);
            for (const delay of RETRY_DELAYS_MS) {
              if (!isActivationRefusal(result)) break;
              await sleep(delay, signal);
              result = await request("tools/call", { name: tool.name, arguments: parameters }, signal);
            }
            const content = Array.isArray(result.content) ? result.content : [];
            return {
              content: piContent(content),
              details: {
                content,
                ...(Object.hasOwn(result, "structuredContent") ? { structuredContent: result.structuredContent } : {}),
                isError: result.isError === true,
              },
            };
          },
        });
      }
      initialized = true;
    } catch (error) {
      url = null;
      capability = null;
      catalogDigest = null;
      registeredNames.clear();
      context.shutdown();
      throw error;
    }
  });

  pi.on("tool_result", (event) => {
    if (!registeredNames.has(String(event.toolName)) || !object(event.details)) return;
    if (event.details.isError === true) return { isError: true };
  });
  pi.on("session_shutdown", () => {
    url = null;
    capability = null;
    catalogDigest = null;
    initialized = false;
    registeredNames.clear();
  });
}
