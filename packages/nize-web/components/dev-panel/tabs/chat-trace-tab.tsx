"use client";

// @zen-component: TRC-ChatTraceTab

/**
 * Dev panel tab for displaying chat trace data.
 * Connects to SSE endpoint when conversation is active.
 */

import { useState, useEffect, useRef, useCallback } from "react";
import { useDevPanel } from "@/lib/dev-panel-context";
import { apiUrl } from "@/lib/api";

// Trace event types matching backend
interface TraceEvent {
  type: string;
  timestamp: string;
  messageId: string;
  payload: Record<string, unknown>;
}

interface PromptSection {
  name: string;
  content: string;
  startIndex: number;
  endIndex: number;
}

interface PromptBreakdown {
  sections: PromptSection[];
  fullPrompt: string;
}

// @zen-impl: TRC-5_AC-1, TRC-5_AC-2, TRC-5_AC-6
export function ChatTraceTab() {
  const { conversationId, isAdmin, traceKey } = useDevPanel();
  const [events, setEvents] = useState<TraceEvent[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedSections, setExpandedSections] = useState<Set<string>>(new Set());
  const eventSourceRef = useRef<EventSource | null>(null);

  // Toggle section expansion - must be before any conditional returns
  const toggleSection = useCallback((key: string) => {
    setExpandedSections((prev) => {
      const next = new Set(prev);
      if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
  }, []);

  // Clear events only when conversation changes, not on traceKey change
  useEffect(() => {
    setEvents([]);
    setError(null);
  }, [conversationId]);

  // @zen-impl: TRC-5_AC-3 - SSE connection for live updates
  // Must be before any conditional returns to satisfy React's Rules of Hooks
  // traceKey changes when new chat activity starts, triggering reconnection
  useEffect(() => {
    if (!conversationId || !isAdmin) return;

    // Small delay to let backend set up the trace emitter
    const delayMs = traceKey > 0 ? 100 : 0;

    const timeoutId = setTimeout(() => {
      setIsLoading(true);
      setError(null);

      // Connect to SSE endpoint
      const url = apiUrl(`/dev/chat_trace?conversationId=${conversationId}`);
      const eventSource = new EventSource(url, { withCredentials: true });
      eventSourceRef.current = eventSource;

      eventSource.addEventListener("trace", (e) => {
        try {
          const event = JSON.parse(e.data) as TraceEvent;
          // Append new events, avoiding duplicates by messageId+type+timestamp
          setEvents((prev) => {
            const key = `${event.messageId}-${event.type}-${event.timestamp}`;
            const exists = prev.some((p) => `${p.messageId}-${p.type}-${p.timestamp}` === key);
            if (exists) return prev;
            return [...prev, event];
          });
          setIsLoading(false);
        } catch {
          console.error("[ChatTraceTab] Failed to parse trace event");
        }
      });

      eventSource.addEventListener("done", () => {
        setIsLoading(false);
        eventSource.close();
      });

      eventSource.addEventListener("error", () => {
        setIsLoading(false);
        // Only set error if we didn't get any events (404 case)
        setEvents((prev) => {
          if (prev.length === 0) {
            setError("No trace data available");
          }
          return prev;
        });
        eventSource.close();
      });

      eventSource.onerror = () => {
        setIsLoading(false);
      };
    }, delayMs);

    return () => {
      clearTimeout(timeoutId);
      if (eventSourceRef.current) {
        eventSourceRef.current.close();
        eventSourceRef.current = null;
      }
    };
  }, [conversationId, isAdmin, traceKey]);

  // @zen-impl: TRC-5_AC-4 - Admin-required message
  if (!isAdmin) {
    return (
      <div className="flex items-center justify-center h-32 text-gray-400 text-sm">
        <div className="text-center">
          <div className="text-lg mb-2">🔒</div>
          <div>Admin access required</div>
          <div className="text-xs mt-1 text-gray-500">Trace data is only available to administrators</div>
        </div>
      </div>
    );
  }

  // @zen-impl: TRC-5_AC-5 - Empty conversation state
  if (!conversationId) {
    return (
      <div className="flex items-center justify-center h-32 text-gray-400 text-sm">
        <div className="text-center">
          <div className="text-lg mb-2">💬</div>
          <div>No active conversation</div>
          <div className="text-xs mt-1 text-gray-500">Start a chat to see trace data</div>
        </div>
      </div>
    );
  }

  // Render event content based on type
  const renderEventContent = (event: TraceEvent, index: number) => {
    const key = `${event.type}-${index}`;
    const isExpanded = expandedSections.has(key);

    return (
      <div key={key} className="border border-gray-700 rounded mb-2">
        <button onClick={() => toggleSection(key)} className="w-full px-3 py-2 flex items-center justify-between hover:bg-gray-800 transition-colors text-left">
          <div className="flex items-center gap-2">
            <span className="text-xs font-mono text-blue-400">{event.type}</span>
            <span className="text-xs text-gray-500">{new Date(event.timestamp).toLocaleTimeString()}</span>
          </div>
          <span className="text-gray-500">{isExpanded ? "▼" : "▶"}</span>
        </button>

        {isExpanded && <div className="px-3 py-2 border-t border-gray-700 bg-gray-800/50">{event.type === "prompt_construction" ? renderPromptBreakdown(event.payload as { breakdown: PromptBreakdown }) : <pre className="text-xs overflow-x-auto whitespace-pre-wrap text-green-400">{JSON.stringify(event.payload, null, 2)}</pre>}</div>}
      </div>
    );
  };

  // @zen-impl: TRC-5_AC-6 - Render prompt breakdown with formatting
  const renderPromptBreakdown = (payload: { breakdown: PromptBreakdown }) => {
    const { breakdown } = payload;
    if (!breakdown?.sections) return <pre className="text-xs text-green-400">{JSON.stringify(payload, null, 2)}</pre>;

    return (
      <div className="space-y-2">
        <div className="text-xs text-gray-400 mb-2">Prompt Sections ({breakdown.sections.length})</div>
        {breakdown.sections.map((section, i) => (
          <div key={i} className="border border-gray-600 rounded">
            <div className="px-2 py-1 bg-gray-700 text-xs font-mono text-yellow-400 flex justify-between">
              <span>{section.name}</span>
              <span className="text-gray-500">
                {section.startIndex}-{section.endIndex}
              </span>
            </div>
            <pre className="px-2 py-1 text-xs overflow-x-auto whitespace-pre-wrap text-gray-300 max-h-40 overflow-y-auto">{section.content.length > 500 ? section.content.slice(0, 500) + "..." : section.content}</pre>
          </div>
        ))}
        <div className="text-xs text-gray-500">Total prompt length: {breakdown.fullPrompt?.length ?? 0} chars</div>
      </div>
    );
  };

  // Group events by type for summary
  const eventCounts = events.reduce(
    (acc, event) => {
      acc[event.type] = (acc[event.type] || 0) + 1;
      return acc;
    },
    {} as Record<string, number>,
  );

  return (
    <div className="space-y-4">
      {/* Header with conversation ID */}
      <div className="flex items-center justify-between">
        <div className="text-xs text-gray-400">
          Conversation: <span className="font-mono text-gray-300">{conversationId.slice(0, 8)}...</span>
        </div>
        {isLoading && <div className="text-xs text-blue-400 animate-pulse">Streaming...</div>}
      </div>

      {/* Event summary */}
      {events.length > 0 && (
        <div className="flex flex-wrap gap-2 text-xs">
          {Object.entries(eventCounts).map(([type, count]) => (
            <span key={type} className="px-2 py-1 bg-gray-700 rounded text-gray-300">
              {type}: {count}
            </span>
          ))}
        </div>
      )}

      {/* Error state */}
      {error && (
        <div className="text-center py-4 text-gray-400 text-sm">
          <div className="text-lg mb-2">📭</div>
          <div>{error}</div>
        </div>
      )}

      {/* Events list */}
      {events.length > 0 && <div className="space-y-1">{events.map((event, i) => renderEventContent(event, i))}</div>}

      {/* Empty state after loading */}
      {!isLoading && !error && events.length === 0 && (
        <div className="text-center py-4 text-gray-400 text-sm">
          <div className="text-lg mb-2">⏳</div>
          <div>Waiting for trace events...</div>
          <div className="text-xs mt-1 text-gray-500">Send a message to generate trace data</div>
        </div>
      )}
    </div>
  );
}
