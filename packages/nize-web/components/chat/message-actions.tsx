// @awa-component: CHAT-MessageActions

"use client";

import { useState } from "react";
import { useCopyToClipboard } from "usehooks-ts";
import { toast } from "sonner";
import { Copy, Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger, TooltipProvider } from "@/components/ui/tooltip";
import type { UIMessage } from "ai";
import { extractTextContent } from "@/lib/message-parts";

interface MessageActionsProps {
  message: UIMessage;
}

// @awa-impl: CHAT-7.1_AC-2
// @awa-impl: CHAT-7.1_AC-3
// @awa-impl: CHAT-7.1_AC-4
export function MessageActions({ message }: MessageActionsProps) {
  const [, copy] = useCopyToClipboard();
  const [showCopied, setShowCopied] = useState(false);

  // Only show actions for assistant messages
  if (message.role !== "assistant") {
    return null;
  }

  const handleCopy = async () => {
    const textContent = extractTextContent(message);
    if (!textContent) {
      toast.error("No text content to copy");
      return;
    }

    const success = await copy(textContent);
    if (success) {
      setShowCopied(true);
      toast.success("Copied to clipboard");
      setTimeout(() => setShowCopied(false), 2000);
    } else {
      toast.error("Failed to copy");
    }
  };

  return (
    <div className="flex items-center gap-1 mt-1 opacity-0 group-hover:opacity-100 transition-opacity">
      <TooltipProvider delayDuration={300}>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button variant="ghost" size="icon" onClick={handleCopy} className="h-7 w-7 text-gray-400 hover:text-gray-600" aria-label={showCopied ? "Copied" : "Copy message"}>
              {showCopied ? <Check className="h-3.5 w-3.5 text-green-500" /> : <Copy className="h-3.5 w-3.5" />}
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            <p>{showCopied ? "Copied!" : "Copy message"}</p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </div>
  );
}
