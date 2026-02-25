// @awa-component: CHAT-Input

interface ChatInputProps {
  /** Current input value */
  value: string;
  /** Called when input changes */
  onChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
  /** Called when form is submitted */
  onSubmit: (event: React.FormEvent<HTMLFormElement>) => void;
  /** Whether chat is loading/streaming */
  isLoading: boolean;
}

export function ChatInput({ value, onChange, onSubmit, isLoading }: ChatInputProps) {
  return (
    <form onSubmit={onSubmit} className="border-t bg-white p-4">
      <div className="mx-auto max-w-3xl flex gap-2">
        <input type="text" value={value} onChange={onChange} placeholder="Ask about your data..." disabled={isLoading} className="flex-1 rounded-lg border border-gray-300 px-4 py-2 focus:outline-none focus:ring-2 focus:ring-blue-500 disabled:opacity-50" />
        <button type="submit" disabled={isLoading || !value.trim()} className="rounded-lg bg-blue-600 px-6 py-2 text-white hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed">
          Send
        </button>
      </div>
    </form>
  );
}
