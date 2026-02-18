// @zen-component: CHAT-ToolRenderer

"use client";

import { useState } from "react";
import { Loader2, ChevronDown, ChevronRight, AlertCircle, CheckCircle2, AlertTriangle } from "lucide-react";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import type { ToolInvocationPart, ToolState } from "@/lib/message-parts";
import { isToolLoading, isToolError } from "@/lib/message-parts";

interface ToolRendererProps {
  part: ToolInvocationPart;
}

// @zen-impl: RSL-2.1_AC-1, RSL-2.1_AC-2
interface TruncatedResponse {
  readonly type: "response_truncated";
  readonly toolName: string;
  readonly actualSize: number;
  readonly limit: number;
  readonly preview: string;
  readonly message: string;
}

function isTruncatedResponse(value: unknown): value is TruncatedResponse {
  return typeof value === "object" && value !== null && "type" in value && (value as TruncatedResponse).type === "response_truncated";
}

// @zen-impl: CHAT-7.2_AC-1
// @zen-impl: CHAT-7.2_AC-2
// @zen-impl: CHAT-7.2_AC-3
// @zen-impl: CHAT-7.2_AC-4
// @zen-impl: CHAT-7.2_AC-5
export function ToolRenderer({ part }: ToolRendererProps) {
  const [isOpen, setIsOpen] = useState(false);
  const { type, toolName: explicitToolName, input, state, output, errorText } = part;

  // Extract tool name: use explicit toolName if provided, otherwise extract from type "tool-{name}"
  const rawToolName = explicitToolName ?? (type.startsWith("tool-") ? type.slice(5) : type);

  // For execute_tool, get the actual tool name from input (available during streaming) or output (after completion)
  const inputObj = input as { toolName?: string } | undefined;
  const outputObj = output as { toolName?: string; result?: unknown } | undefined;
  const actualToolName = rawToolName === "execute_tool" ? (inputObj?.toolName ?? outputObj?.toolName) : undefined;

  // Check if output is a truncated response
  const resultValue = outputObj?.result ?? output;
  const truncatedResponse = isTruncatedResponse(resultValue) ? resultValue : null;
  const isTruncated = truncatedResponse !== null;

  // Build display name: "Execute Tool [actual_tool]" or just the formatted tool name
  const displayName = actualToolName ? `Execute Tool [${formatToolName(actualToolName)}]` : formatToolName(rawToolName);

  return (
    <div className={`my-2 rounded-lg border ${isTruncated ? "border-amber-200 bg-amber-50" : "border-gray-200 bg-gray-50"}`}>
      <Collapsible open={isOpen} onOpenChange={setIsOpen}>
        <CollapsibleTrigger className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-gray-100 rounded-lg transition-colors">
          {/* Status icon */}
          <ToolStatusIcon state={state} isTruncated={isTruncated} />

          {/* Tool name */}
          <span className="font-medium text-gray-700 text-sm">{displayName}</span>

          {/* Status text */}
          <span className="text-xs text-gray-500">
            <ToolStatusText state={state} isTruncated={isTruncated} />
          </span>

          {/* Expand indicator */}
          <span className="ml-auto text-gray-400">{isOpen ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}</span>
        </CollapsibleTrigger>

        <CollapsibleContent>
          <div className="border-t border-gray-200 px-3 py-2 space-y-2">
            {/* Input arguments */}
            {input !== undefined && input !== null && (
              <div>
                <div className="text-xs font-medium text-gray-500 mb-1">Input</div>
                <pre className="text-xs bg-white rounded p-2 overflow-x-auto border border-gray-100">{formatJSON(input)}</pre>
              </div>
            )}

            {/* Truncated response warning */}
            {state === "output-available" && truncatedResponse && (
              <div>
                <div className="text-xs font-medium text-amber-600 mb-1">Response Truncated</div>
                <div className="text-xs bg-amber-100 text-amber-800 rounded p-2 border border-amber-200 mb-2">{truncatedResponse.message}</div>
                <div className="text-xs font-medium text-gray-500 mb-1">Preview</div>
                <pre className="text-xs bg-white rounded p-2 overflow-x-auto border border-gray-100 max-h-48 overflow-y-auto">{truncatedResponse.preview}</pre>
              </div>
            )}

            {/* Output result - only show when available and not truncated */}
            {state === "output-available" && output !== undefined && !truncatedResponse && (
              <div>
                <div className="text-xs font-medium text-gray-500 mb-1">Output</div>
                <pre className="text-xs bg-white rounded p-2 overflow-x-auto border border-gray-100">{formatJSON(output)}</pre>
              </div>
            )}

            {/* Error message */}
            {state === "output-error" && errorText && (
              <div>
                <div className="text-xs font-medium text-red-500 mb-1">Error</div>
                <pre className="text-xs bg-red-50 text-red-700 rounded p-2 overflow-x-auto border border-red-100">{errorText}</pre>
              </div>
            )}
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}

/**
 * Status icon based on tool state
 */
function ToolStatusIcon({ state, isTruncated }: { state: ToolState; isTruncated?: boolean }) {
  if (isToolLoading(state)) {
    return <Loader2 className="h-4 w-4 text-blue-500 animate-spin" />;
  }

  if (isToolError(state)) {
    return <AlertCircle className="h-4 w-4 text-red-500" />;
  }

  if (isTruncated) {
    return <AlertTriangle className="h-4 w-4 text-amber-500" />;
  }

  return <CheckCircle2 className="h-4 w-4 text-green-500" />;
}

/**
 * Status text based on tool state
 */
function ToolStatusText({ state, isTruncated }: { state: ToolState; isTruncated?: boolean }) {
  switch (state) {
    case "input-streaming":
      return "Preparing...";
    case "input-available":
      return "Running...";
    case "output-available":
      return isTruncated ? "Truncated" : "Complete";
    case "output-error":
      return "Failed";
    default:
      return "";
  }
}

/**
 * Format tool name for display
 * Converts snake_case or camelCase to Title Case
 */
function formatToolName(name: string): string {
  return name
    .replace(/_/g, " ")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

/**
 * Format JSON for display
 */
function formatJSON(value: unknown): string {
  try {
    return JSON.stringify(value, null, 2);
  } catch {
    return String(value);
  }
}
