// @awa-test: CHAT_P-7
// @awa-test: CHAT-7.1_AC-1
// @awa-test: CHAT-7.1_AC-2
// @awa-test: CHAT-7.1_AC-3

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, within } from "@testing-library/react";

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

import { CodeBlock } from "../components/chat/code-block";

describe("CodeBlock", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockCopy.mockResolvedValue(true);
  });

  it("renders code content", () => {
    render(<CodeBlock>const x = 1;</CodeBlock>);

    expect(screen.getByText("const x = 1;")).toBeInTheDocument();
  });

  it("displays language label when provided", () => {
    render(<CodeBlock language="typescript">const x: number = 1;</CodeBlock>);

    expect(screen.getByText("typescript")).toBeInTheDocument();
  });

  // @awa-test: CHAT-7.1_AC-1
  it("copy button has group-hover visibility class", () => {
    const { container } = render(<CodeBlock language="javascript">console.log('test');</CodeBlock>);

    // Find the button by aria-label within the container
    const copyButton = container.querySelector('button[aria-label="Copy code"]');
    expect(copyButton).toBeInTheDocument();
    expect(copyButton).toHaveClass("opacity-0");
    expect(copyButton).toHaveClass("group-hover:opacity-100");
  });

  // @awa-test: CHAT_P-7
  // @awa-test: CHAT-7.1_AC-2
  it("copies exact code content to clipboard on click", async () => {
    const codeContent = 'function test() {\n  return "hello";\n}';
    const { container } = render(<CodeBlock>{codeContent}</CodeBlock>);

    const copyButton = container.querySelector('button[aria-label="Copy code"]');
    expect(copyButton).not.toBeNull();
    fireEvent.click(copyButton!);

    await waitFor(() => {
      expect(mockCopy).toHaveBeenCalledWith(codeContent);
    });
  });

  // @awa-test: CHAT-7.1_AC-3
  it("shows success toast on successful copy", async () => {
    mockCopy.mockResolvedValue(true);

    const { container } = render(<CodeBlock>test code</CodeBlock>);

    const copyButton = container.querySelector('button[aria-label="Copy code"]');
    expect(copyButton).not.toBeNull();
    fireEvent.click(copyButton!);

    await waitFor(() => {
      expect(mockToastSuccess).toHaveBeenCalledWith("Copied to clipboard");
    });
  });

  it("shows error toast on failed copy", async () => {
    mockCopy.mockResolvedValue(false);

    const { container } = render(<CodeBlock>test code</CodeBlock>);

    const copyButton = container.querySelector('button[aria-label="Copy code"]');
    expect(copyButton).not.toBeNull();
    fireEvent.click(copyButton!);

    await waitFor(() => {
      expect(mockToastError).toHaveBeenCalledWith("Failed to copy");
    });
  });

  it("shows check icon after successful copy", async () => {
    mockCopy.mockResolvedValue(true);

    const { container } = render(<CodeBlock>test code</CodeBlock>);

    const copyButton = container.querySelector('button[aria-label="Copy code"]');
    expect(copyButton).not.toBeNull();
    fireEvent.click(copyButton!);

    await waitFor(() => {
      const copiedButton = container.querySelector('button[aria-label="Copied"]');
      expect(copiedButton).toBeInTheDocument();
    });
  });

  it("applies correct styling without language label", () => {
    render(<CodeBlock>plain code</CodeBlock>);

    const pre = screen.getByText("plain code").closest("pre");
    expect(pre).toHaveClass("rounded-lg");
  });

  it("applies correct styling with language label", () => {
    render(<CodeBlock language="python">print("hello")</CodeBlock>);

    const pre = screen.getByText('print("hello")').closest("pre");
    expect(pre).toHaveClass("rounded-b-lg");
  });
});
