// @zen-component: CHAT-MessageBubble

"use client";

import type { UIMessage } from "ai";
import { Streamdown, streamdownPlugins, streamdownComponents } from "@/lib/streamdown-config";
import { isTextPart, isReasoningPart, isToolInvocationPart, type ToolInvocationPart } from "@/lib/message-parts";
import { ToolRenderer } from "./tool-renderer";
import { MessageActions } from "./message-actions";
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "@/components/ui/collapsible";
import { ChevronDown, ChevronRight, Brain } from "lucide-react";
import { useState } from "react";
import { cn } from "@/lib/utils";

interface MessageBubbleProps {
  message: UIMessage;
  isStreaming?: boolean;
}

// @zen-impl: CHAT-7_AC-1
// @zen-impl: CHAT-7_AC-2
// @zen-impl: CHAT-7_AC-4
export function MessageBubble({ message, isStreaming = false }: MessageBubbleProps) {
  const isUser = message.role === "user";

  return (
    <div className={cn("flex group", isUser ? "justify-end" : "justify-start")}>
      <div className="flex flex-col max-w-[85%]">
        <div className={cn("rounded-lg px-4 py-2", isUser ? "bg-blue-600 text-white" : "bg-gray-100 text-gray-900")}>
          <MessageContent message={message} isStreaming={isStreaming} isUser={isUser} />
        </div>
        {/* Message actions (copy button) for assistant messages */}
        <MessageActions message={message} />
      </div>
    </div>
  );
}

/**
 * Renders message content based on parts array
 */
function MessageContent({ message, isStreaming, isUser }: { message: UIMessage; isStreaming: boolean; isUser: boolean }) {
  // If no parts, fall back to legacy content field
  if (!message.parts || message.parts.length === 0) {
    const content = (message as unknown as { content?: string }).content ?? "";
    return <p className="whitespace-pre-wrap min-h-[1.5em]">{content}</p>;
  }

  return (
    <div className="space-y-2">
      {message.parts.map((part, index) => (
        <MessagePart key={`${message.id}-part-${index}`} part={part} isStreaming={isStreaming} isUser={isUser} />
      ))}
    </div>
  );
}

/**
 * Renders individual message part by type
 */
function MessagePart({ part, isStreaming, isUser }: { part: unknown; isStreaming: boolean; isUser: boolean }) {
  // Text part - render with Streamdown for markdown
  if (isTextPart(part)) {
    return (
      <div className={cn("prose prose-sm max-w-none", isUser ? "prose-invert" : "")}>
        <Streamdown plugins={streamdownPlugins} components={streamdownComponents} isAnimating={isStreaming}>
          {part.text}
        </Streamdown>
      </div>
    );
  }

  // Reasoning part - collapsible thinking section
  if (isReasoningPart(part)) {
    return <ReasoningSection reasoning={part.reasoning} />;
  }

  // Tool invocation part - delegate to ToolRenderer
  if (isToolInvocationPart(part)) {
    return <ToolRenderer part={part as ToolInvocationPart} />;
  }

  // Unknown part type - ignore
  return null;
}

/**
 * Collapsible reasoning/thinking section
 */
function ReasoningSection({ reasoning }: { reasoning: string }) {
  const [isOpen, setIsOpen] = useState(false);

  return (
    <div className="my-2 rounded-lg border border-purple-200 bg-purple-50">
      <Collapsible open={isOpen} onOpenChange={setIsOpen}>
        <CollapsibleTrigger className="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-purple-100 rounded-lg transition-colors">
          <Brain className="h-4 w-4 text-purple-500" />
          <span className="text-sm font-medium text-purple-700">Thinking</span>
          <span className="ml-auto text-purple-400">{isOpen ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}</span>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <div className="border-t border-purple-200 px-3 py-2">
            <p className="text-sm text-purple-800 whitespace-pre-wrap">{reasoning}</p>
          </div>
        </CollapsibleContent>
      </Collapsible>
    </div>
  );
}

export function ThinkingBubble() {
  return (
    <div className="flex justify-start">
      <div className="bg-gray-200 rounded-lg px-4 py-2 text-gray-900">
        <p>Thinking...</p>
      </div>
    </div>
  );
}

export function EmptyState() {
  return (
    <div className="text-center text-gray-500">
      <p className="text-lg">Welcome to Nize!</p>
      <p className="text-sm">Upload files and chat with your data.</p>
    </div>
  );
}
