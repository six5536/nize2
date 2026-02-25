// @awa-component: CHAT-MessageBubble
// Streamdown configuration for markdown rendering with syntax highlighting

import type { ReactNode, ComponentType, JSX } from "react";
import { Streamdown, type PluginConfig } from "streamdown";
import { code } from "@streamdown/code";
import { CodeBlock, InlineCode } from "@/components/chat/code-block";

// Components type from streamdown (not exported directly)
type Components = {
  [Key in keyof JSX.IntrinsicElements]?: ComponentType<JSX.IntrinsicElements[Key]> | keyof JSX.IntrinsicElements;
};

/**
 * Streamdown plugins configuration for code block syntax highlighting
 * Uses Shiki under the hood via @streamdown/code
 */
export const streamdownPlugins: PluginConfig = {
  code,
};

/**
 * Custom components for Streamdown to use for rendering
 */
export const streamdownComponents: Components = {
  // Override code blocks to use our custom CodeBlock with copy button.
  // Fenced code blocks receive "data-block" from streamdown's built-in pre handler;
  // inline code does not and must stay inline to avoid <p> > <div>/<pre> nesting.
  code: ({ children, className, ...rest }: { children?: ReactNode; className?: string; [key: string]: unknown }) => {
    if ("data-block" in rest) {
      const language = className?.replace("language-", "") ?? "";
      const codeContent = typeof children === "string" ? children : String(children);
      return <CodeBlock language={language}>{codeContent}</CodeBlock>;
    }
    return <InlineCode>{children}</InlineCode>;
  },
  // Style links for accessibility
  a: ({ href, children }: { href?: string; children?: ReactNode }) => (
    <a href={href} target="_blank" rel="noopener noreferrer" className="text-blue-600 hover:underline">
      {children}
    </a>
  ),
};

export { Streamdown };
