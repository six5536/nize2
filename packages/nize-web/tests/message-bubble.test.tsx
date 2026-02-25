// @awa-test: CHAT_P-5
// @awa-test: CHAT-7_AC-1
// @awa-test: CHAT-7_AC-2

import React from "react";
import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, cleanup, within, fireEvent } from "@testing-library/react";
import type { UIMessage } from "ai";

// Mock streamdown since it has complex dependencies
vi.mock("@/lib/streamdown-config", () => ({
  Streamdown: ({ children }: { children: string }) => <div data-testid="streamdown">{children}</div>,
  streamdownPlugins: {},
  streamdownComponents: {},
}));

// Mock the sub-components
vi.mock("../components/chat/tool-renderer", () => ({
  ToolRenderer: ({ part }: { part: { type: string; toolName?: string } }) => {
    // Extract tool name: use explicit toolName if provided, otherwise extract from type "tool-{name}"
    const toolName = part.toolName ?? (part.type.startsWith("tool-") ? part.type.slice(5) : part.type);
    return <div data-testid="tool-renderer">{toolName}</div>;
  },
}));

vi.mock("../components/chat/message-actions", () => ({
  MessageActions: () => <div data-testid="message-actions" />,
}));

import { MessageBubble, ThinkingBubble, EmptyState } from "../components/chat/message-bubble";

describe("MessageBubble", () => {
  // Cleanup after each test to ensure isolation
  afterEach(() => {
    cleanup();
  });

  // @awa-test: CHAT-7_AC-1
  it("renders message parts array correctly", () => {
    const message: UIMessage = {
      id: "test-1",
      role: "assistant",
      parts: [
        { type: "text", text: "Hello world" },
        { type: "text", text: "Second part" },
      ],
    };

    render(<MessageBubble message={message} />);

    expect(screen.getByText("Hello world")).toBeInTheDocument();
    expect(screen.getByText("Second part")).toBeInTheDocument();
  });

  // @awa-test: CHAT_P-5
  it("preserves text content semantically through markdown rendering", () => {
    const markdownText = "# Heading\n\nParagraph with **bold** text";
    const message: UIMessage = {
      id: "test-2",
      role: "assistant",
      parts: [{ type: "text", text: markdownText }],
    };

    render(<MessageBubble message={message} />);

    // The text should be passed to Streamdown component
    const streamdowns = screen.getAllByTestId("streamdown");
    expect(streamdowns.some((el) => el.textContent === markdownText)).toBe(true);
  });

  // @awa-test: CHAT-7_AC-4
  it("distinguishes user messages from assistant messages visually", () => {
    const userMessage: UIMessage = {
      id: "user-1",
      role: "user",
      parts: [{ type: "text", text: "User message" }],
    };

    const { container } = render(<MessageBubble message={userMessage} />);

    // User messages should have justify-end on the outer flex container
    const outerDiv = container.querySelector(".flex.group");
    expect(outerDiv).toHaveClass("justify-end");

    // And should have blue background
    const innerBubble = container.querySelector(".bg-blue-600");
    expect(innerBubble).toBeInTheDocument();
  });

  it("renders assistant messages with proper styling", () => {
    const assistantMessage: UIMessage = {
      id: "assistant-1",
      role: "assistant",
      parts: [{ type: "text", text: "Assistant message" }],
    };

    const { container } = render(<MessageBubble message={assistantMessage} />);

    // Assistant messages should have justify-start
    const outerDiv = container.querySelector(".flex.group");
    expect(outerDiv).toHaveClass("justify-start");

    // And should have gray background
    const innerBubble = container.querySelector(".bg-gray-100");
    expect(innerBubble).toBeInTheDocument();
  });

  it("renders tool invocation parts using ToolRenderer", () => {
    const message: UIMessage = {
      id: "test-3",
      role: "assistant",
      parts: [
        {
          type: "tool-search",
          toolCallId: "tool-1",
          input: { query: "test" },
          state: "output-available",
        },
      ],
    };

    render(<MessageBubble message={message} />);

    expect(screen.getByTestId("tool-renderer")).toBeInTheDocument();
    // Tool name is extracted from type "tool-search" -> "search" (lowercase in mock)
    expect(screen.getByText("search")).toBeInTheDocument();
  });

  it("renders reasoning parts as collapsible sections", () => {
    const message: UIMessage = {
      id: "test-4",
      role: "assistant",
      parts: [{ type: "reasoning", reasoning: "Let me think about this..." }],
    };

    const { container } = render(<MessageBubble message={message} />);

    expect(screen.getByText("Thinking")).toBeInTheDocument();

    // The reasoning content is inside a collapsible, click to expand
    const button = container.querySelector("button");
    expect(button).not.toBeNull();
    fireEvent.click(button!);

    // After expanding, we should see the reasoning content
    expect(screen.getByText("Let me think about this...")).toBeInTheDocument();
  });

  it("falls back to legacy content field when parts is empty", () => {
    const message = {
      id: "test-5",
      role: "assistant",
      parts: [],
      content: "Legacy content",
    } as unknown as UIMessage;

    render(<MessageBubble message={message} />);

    expect(screen.getByText("Legacy content")).toBeInTheDocument();
  });

  it("shows message actions for assistant messages", () => {
    const message: UIMessage = {
      id: "test-6",
      role: "assistant",
      parts: [{ type: "text", text: "Hello" }],
    };

    const { container } = render(<MessageBubble message={message} />);

    // Find within this specific render
    const messageActions = container.querySelector('[data-testid="message-actions"]');
    expect(messageActions).toBeInTheDocument();
  });
});

describe("ThinkingBubble", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders thinking indicator", () => {
    render(<ThinkingBubble />);
    expect(screen.getByText("Thinking...")).toBeInTheDocument();
  });
});

describe("EmptyState", () => {
  afterEach(() => {
    cleanup();
  });

  it("renders welcome message", () => {
    render(<EmptyState />);
    expect(screen.getByText("Welcome to Nize!")).toBeInTheDocument();
    expect(screen.getByText("Upload files and chat with your data.")).toBeInTheDocument();
  });
});
