// @zen-test: CHAT-7.1_AC-2
// @zen-test: CHAT-7.1_AC-3
// @zen-test: CHAT-7.1_AC-4

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import type { UIMessage } from "ai";

// Create mock functions that can be spied on
const mockCopy = vi.fn();
const mockToastSuccess = vi.fn();
const mockToastError = vi.fn();

// Mock sonner toast - must define factory inline
vi.mock("sonner", () => ({
  toast: {
    success: (...args: unknown[]) => mockToastSuccess(...args),
    error: (...args: unknown[]) => mockToastError(...args),
  },
}));

// Mock the clipboard API
vi.mock("usehooks-ts", () => ({
  useCopyToClipboard: () => [null, (...args: unknown[]) => mockCopy(...args)],
}));

import { MessageActions } from "../components/chat/message-actions";

describe("MessageActions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCopy.mockResolvedValue(true);
  });

  const createAssistantMessage = (text: string): UIMessage => ({
    id: "test-1",
    role: "assistant",
    parts: [{ type: "text", text }],
  });

  const createUserMessage = (text: string): UIMessage => ({
    id: "test-2",
    role: "user",
    parts: [{ type: "text", text }],
  });

  it("renders copy button for assistant messages", () => {
    const message = createAssistantMessage("Hello world");
    const { container } = render(<MessageActions message={message} />);

    const copyButton = container.querySelector('button[aria-label="Copy message"]');
    expect(copyButton).toBeInTheDocument();
  });

  it("returns null for user messages", () => {
    const message = createUserMessage("User message");
    const { container } = render(<MessageActions message={message} />);

    expect(container).toBeEmptyDOMElement();
  });

  // @zen-test: CHAT-7.1_AC-2
  it("copies message content to clipboard on click", async () => {
    const content = "This is the message content to copy";
    const message = createAssistantMessage(content);
    const { container } = render(<MessageActions message={message} />);

    const copyButton = container.querySelector('button[aria-label="Copy message"]');
    expect(copyButton).not.toBeNull();
    fireEvent.click(copyButton!);

    await waitFor(() => {
      expect(mockCopy).toHaveBeenCalledWith(content);
    });
  });

  // @zen-test: CHAT-7.1_AC-3
  it("shows success toast on successful copy", async () => {
    mockCopy.mockResolvedValue(true);
    const message = createAssistantMessage("Test content");
    const { container } = render(<MessageActions message={message} />);

    const copyButton = container.querySelector('button[aria-label="Copy message"]');
    expect(copyButton).not.toBeNull();
    fireEvent.click(copyButton!);

    await waitFor(() => {
      expect(mockToastSuccess).toHaveBeenCalledWith("Copied to clipboard");
    });
  });

  it("shows error toast on failed copy", async () => {
    mockCopy.mockResolvedValue(false);
    const message = createAssistantMessage("Test content");
    const { container } = render(<MessageActions message={message} />);

    const copyButton = container.querySelector('button[aria-label="Copy message"]');
    expect(copyButton).not.toBeNull();
    fireEvent.click(copyButton!);

    await waitFor(() => {
      expect(mockToastError).toHaveBeenCalledWith("Failed to copy");
    });
  });

  // @zen-test: CHAT-7.1_AC-4
  it("shows visual feedback (check icon) after copy", async () => {
    mockCopy.mockResolvedValue(true);
    const message = createAssistantMessage("Test content");
    const { container } = render(<MessageActions message={message} />);

    const copyButton = container.querySelector('button[aria-label="Copy message"]');
    expect(copyButton).not.toBeNull();
    fireEvent.click(copyButton!);

    await waitFor(() => {
      const copiedButton = container.querySelector('button[aria-label="Copied"]');
      expect(copiedButton).toBeInTheDocument();
    });
  });

  it("reverts to copy icon after 2 seconds", async () => {
    vi.useFakeTimers();
    mockCopy.mockResolvedValue(true);
    const message = createAssistantMessage("Test content");
    const { container } = render(<MessageActions message={message} />);

    const copyButton = container.querySelector('button[aria-label="Copy message"]');
    expect(copyButton).not.toBeNull();
    fireEvent.click(copyButton!);

    // Run all pending timers to process the setTimeout in the click handler
    await vi.runAllTimersAsync();

    // After all timers, it should have the "Copied" state then revert
    await vi.advanceTimersByTimeAsync(2000);

    // Should now be back to "Copy message"
    const resetCopyButton = container.querySelector('button[aria-label="Copy message"]');
    expect(resetCopyButton).toBeInTheDocument();

    vi.useRealTimers();
  });

  it("has correct styling classes", () => {
    const message = createAssistantMessage("Test");
    const { container } = render(<MessageActions message={message} />);

    const copyButton = container.querySelector('button[aria-label="Copy message"]');
    const parentDiv = copyButton?.parentElement;
    expect(parentDiv).toHaveClass("mt-1");
  });
});
