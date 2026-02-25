// @awa-component: CHAT-CodeBlock

"use client";

import { useState } from "react";
import { useCopyToClipboard } from "usehooks-ts";
import { toast } from "sonner";
import { Copy, Check } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tooltip, TooltipContent, TooltipTrigger, TooltipProvider } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

interface CodeBlockProps {
  children: string;
  language?: string;
  className?: string;
}

// @awa-impl: CHAT-7_AC-3
// @awa-impl: CHAT-7.1_AC-1
// @awa-impl: CHAT-7.1_AC-2
// @awa-impl: CHAT-7.1_AC-3
export function CodeBlock({ children, language, className }: CodeBlockProps) {
  const [, copy] = useCopyToClipboard();
  const [showCopied, setShowCopied] = useState(false);

  const handleCopy = async () => {
    const success = await copy(children);
    if (success) {
      setShowCopied(true);
      toast.success("Copied to clipboard");
      setTimeout(() => setShowCopied(false), 2000);
    } else {
      toast.error("Failed to copy");
    }
  };

  return (
    <div className={cn("group relative my-4", className)}>
      {/* Language label */}
      {language && (
        <div className="flex items-center justify-between rounded-t-lg bg-gray-800 px-4 py-2 text-xs text-gray-400">
          <span>{language}</span>
        </div>
      )}

      {/* Code content */}
      <pre className={cn("overflow-x-auto bg-gray-900 p-4 text-sm text-gray-100", language ? "rounded-b-lg" : "rounded-lg")}>
        <code className={language ? `language-${language}` : undefined}>{children}</code>
      </pre>

      {/* Copy button - visible on hover */}
      <TooltipProvider delayDuration={300}>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              onClick={handleCopy}
              className={cn(
                "absolute right-2 top-2 h-8 w-8 opacity-0 transition-opacity group-hover:opacity-100",
                "bg-gray-700/50 hover:bg-gray-700 text-gray-300 hover:text-white",
                language && "top-10", // Offset for language label
              )}
              aria-label={showCopied ? "Copied" : "Copy code"}
            >
              {showCopied ? <Check className="h-4 w-4 text-green-400" /> : <Copy className="h-4 w-4" />}
            </Button>
          </TooltipTrigger>
          <TooltipContent>
            <p>{showCopied ? "Copied!" : "Copy code"}</p>
          </TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </div>
  );
}

/**
 * Inline code styling (not a code block)
 */
export function InlineCode({ children }: { children: React.ReactNode }) {
  return <code className="rounded bg-gray-100 px-1.5 py-0.5 text-sm font-mono text-gray-800">{children}</code>;
}
