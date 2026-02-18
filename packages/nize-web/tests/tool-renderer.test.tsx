// @zen-test: CHAT_P-6
// @zen-test: CHAT-7.2_AC-1
// @zen-test: CHAT-7.2_AC-2
// @zen-test: CHAT-7.2_AC-3
// @zen-test: CHAT-7.2_AC-4
// @zen-test: CHAT-7.2_AC-5

import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent, cleanup, within } from "@testing-library/react";
import { ToolRenderer } from "../components/chat/tool-renderer";
import type { ToolInvocationPart } from "@/lib/message-parts";

describe("ToolRenderer", () => {
  // Ensure cleanup after each test to prevent multiple buttons issue
  afterEach(() => {
    cleanup();
  });

  // @zen-test: CHAT-7.2_AC-1
  it("renders tool name and status", () => {
    const part: ToolInvocationPart = {
      type: "tool-search_documents",
      toolCallId: "tool-1",
      input: { query: "test" },
      state: "output-available",
    };

    const { container } = render(<ToolRenderer part={part} />);

    expect(screen.getByText("Search Documents")).toBeInTheDocument();
    expect(screen.getByText("Complete")).toBeInTheDocument();
  });

  // @zen-test: CHAT-7.2_AC-2
  it("displays loading indicator for input-streaming state", () => {
    const part: ToolInvocationPart = {
      type: "tool-search",
      toolCallId: "tool-1",
      input: { query: "test" },
      state: "input-streaming",
    };

    render(<ToolRenderer part={part} />);

    expect(screen.getByText("Preparing...")).toBeInTheDocument();
    // Loader icon should be rendered with animate-spin class
    const svg = document.querySelector("svg.animate-spin");
    expect(svg).toBeInTheDocument();
  });

  // @zen-test: CHAT-7.2_AC-2
  it("displays loading indicator for input-available state", () => {
    const part: ToolInvocationPart = {
      type: "tool-search",
      toolCallId: "tool-1",
      input: { query: "test" },
      state: "input-available",
    };

    render(<ToolRenderer part={part} />);

    expect(screen.getByText("Running...")).toBeInTheDocument();
  });

  // @zen-test: CHAT_P-6
  // @zen-test: CHAT-7.2_AC-3
  it("displays tool result when state is output-available", async () => {
    const part: ToolInvocationPart = {
      type: "tool-search",
      toolCallId: "tool-1",
      input: { query: "test" },
      state: "output-available",
      output: { documents: ["doc1", "doc2"] },
    };

    const { container } = render(<ToolRenderer part={part} />);

    // Find and click the collapsible trigger
    const trigger = container.querySelector("button[data-state]");
    expect(trigger).not.toBeNull();
    fireEvent.click(trigger!);

    expect(screen.getByText("Output")).toBeInTheDocument();
    expect(screen.getByText(/"documents"/)).toBeInTheDocument();
    expect(screen.getByText(/"doc1"/)).toBeInTheDocument();
  });

  // @zen-test: CHAT-7.2_AC-4
  it("displays error message when state is output-error", async () => {
    const part: ToolInvocationPart = {
      type: "tool-search",
      toolCallId: "tool-1",
      input: { query: "test" },
      state: "output-error",
      errorText: "Something went wrong",
    };

    const { container } = render(<ToolRenderer part={part} />);

    expect(screen.getByText("Failed")).toBeInTheDocument();

    // Expand to see error details
    const trigger = container.querySelector("button[data-state]");
    expect(trigger).not.toBeNull();
    fireEvent.click(trigger!);

    expect(screen.getByText("Error")).toBeInTheDocument();
    expect(screen.getByText("Something went wrong")).toBeInTheDocument();
  });

  // @zen-test: CHAT-7.2_AC-5
  it("renders tool inputs in expandable/collapsible format", async () => {
    const part: ToolInvocationPart = {
      type: "tool-search",
      toolCallId: "tool-1",
      input: { query: "test query", limit: 10 },
      state: "output-available",
      output: { count: 5 },
    };

    const { container } = render(<ToolRenderer part={part} />);

    // Initially collapsed - input details should not be visible
    expect(screen.queryByText("Input")).not.toBeInTheDocument();

    // Click to expand
    const trigger = container.querySelector("button[data-state]");
    expect(trigger).not.toBeNull();
    fireEvent.click(trigger!);

    // Now input should be visible
    expect(screen.getByText("Input")).toBeInTheDocument();
    expect(screen.getByText(/"test query"/)).toBeInTheDocument();
  });

  // @zen-test: CHAT_P-6
  it("renders correct state for each lifecycle phase", () => {
    const states: Array<{ state: ToolInvocationPart["state"]; expectedStatus: string }> = [
      { state: "input-streaming", expectedStatus: "Preparing..." },
      { state: "input-available", expectedStatus: "Running..." },
      { state: "output-available", expectedStatus: "Complete" },
      { state: "output-error", expectedStatus: "Failed" },
    ];

    for (const { state, expectedStatus } of states) {
      const part: ToolInvocationPart = {
        type: "tool-test_tool",
        toolCallId: `tool-${state}`,
        input: {},
        state,
      };

      const { unmount } = render(<ToolRenderer part={part} />);

      expect(screen.getByText(expectedStatus)).toBeInTheDocument();

      unmount();
    }
  });

  it("formats tool names from snake_case to Title Case", () => {
    const part: ToolInvocationPart = {
      type: "tool-get_user_documents",
      toolCallId: "tool-1",
      input: {},
      state: "output-available",
    };

    render(<ToolRenderer part={part} />);

    expect(screen.getByText("Get User Documents")).toBeInTheDocument();
  });

  it("formats tool names from camelCase to Title Case", () => {
    const part: ToolInvocationPart = {
      type: "tool-getUserDocuments",
      toolCallId: "tool-1",
      input: {},
      state: "output-available",
    };

    render(<ToolRenderer part={part} />);

    // camelCase "getUserDocuments" -> "get User Documents" -> "Get User Documents"
    expect(screen.getByText("Get User Documents")).toBeInTheDocument();
  });
});
