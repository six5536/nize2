// @zen-component: CHAT-Header

interface ChatHeaderProps {
  /** User display name or email */
  userName?: string;
  /** Called when user clicks sign out */
  onLogout: () => void;
}

export function ChatHeader({ userName, onLogout }: ChatHeaderProps) {
  return (
    <header className="border-b bg-white px-6 py-4 flex justify-between items-center">
      <div>
        <h1 className="text-xl font-semibold">Nize</h1>
        <p className="text-sm text-gray-500">Your AI-Powered Data Hub</p>
      </div>
      {userName && (
        <button onClick={onLogout} className="text-sm text-gray-600 hover:text-gray-900">
          Sign out
        </button>
      )}
    </header>
  );
}
