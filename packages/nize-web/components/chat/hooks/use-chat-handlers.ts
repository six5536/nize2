// @awa-component: CHAT-Handlers

import { useCallback, useState } from "react";
import { type UIMessage } from "ai";
import { apiUrl } from "@/lib/api";
import { useAuthFetch } from "@/lib/auth-context";
import { nanoid } from "nanoid";

interface UseFileUploadReturn {
  uploading: boolean;
  uploadMessage: string | null;
  handleUpload: (event: React.ChangeEvent<HTMLInputElement>) => Promise<void>;
}

export function useFileUpload(): UseFileUploadReturn {
  const [uploading, setUploading] = useState(false);
  const [uploadMessage, setUploadMessage] = useState<string | null>(null);

  const handleUpload = useCallback(async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    setUploading(true);
    setUploadMessage(null);

    try {
      const formData = new FormData();
      formData.append("file", file);

      const res = await fetch(apiUrl("/ingest"), {
        method: "POST",
        credentials: "include", // Send httpOnly cookies
        body: formData,
      });

      const data = (await res.json()) as { message?: string; error?: string };
      setUploadMessage(res.ok ? data.message || "Upload complete" : data.error || data.message || "Upload failed");
    } catch {
      setUploadMessage("Upload failed");
    } finally {
      setUploading(false);
      event.target.value = "";
    }
  }, []);

  return { uploading, uploadMessage, handleUpload };
}

interface UseChatSubmitOptions {
  input: string;
  setInput: (value: string) => void;
  sendMessage: (options: { text: string }) => Promise<void>;
  setMessages: React.Dispatch<React.SetStateAction<UIMessage[]>>;
}

interface UseChatSubmitReturn {
  handleChatSubmit: (event: React.FormEvent<HTMLFormElement>) => Promise<void>;
}

export function useChatSubmit({ input, setInput, sendMessage, setMessages }: UseChatSubmitOptions): UseChatSubmitReturn {
  const authFetch = useAuthFetch();

  const handleChatSubmit = useCallback(
    async (event: React.FormEvent<HTMLFormElement>) => {
      event.preventDefault();
      const trimmed = input.trim();
      if (!trimmed) return;

      const isListFilesQuery = /\b(list|show)\s+(my\s+)?(files|documents)\b/i.test(trimmed);

      if (!isListFilesQuery) {
        setInput("");
        await sendMessage({ text: trimmed });
        return;
      }

      // Handle "list files" query locally
      setMessages((prev) => [...prev, { id: nanoid(), role: "user", content: trimmed, parts: [] }]);
      setInput("");

      try {
        const res = await authFetch("/ingest");
        const data = (await res.json()) as { documents?: Array<{ filename: string }> };
        const files = data.documents?.map((d) => d.filename) || [];
        const message = res.ok && files.length > 0 ? `Your uploaded files (most recent first):\n${files.map((file) => `- ${file}`).join("\n")}` : "You have no uploaded files yet.";
        setMessages((prev) => [...prev, { id: nanoid(), role: "assistant", content: message, parts: [] }]);
      } catch {
        setMessages((prev) => [...prev, { id: nanoid(), role: "assistant", content: "Unable to list files right now.", parts: [] }]);
      }
    },
    [input, setInput, sendMessage, setMessages, authFetch],
  );

  return { handleChatSubmit };
}
