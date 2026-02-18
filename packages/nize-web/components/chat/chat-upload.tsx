// @zen-component: CHAT-Upload

interface ChatUploadProps {
  /** Called when file is selected */
  onUpload: (event: React.ChangeEvent<HTMLInputElement>) => void;
  /** Whether upload is in progress */
  uploading: boolean;
  /** Status message to display */
  uploadMessage: string | null;
}

export function ChatUpload({ onUpload, uploading, uploadMessage }: ChatUploadProps) {
  return (
    <div className="rounded-lg border border-dashed border-gray-300 p-4 text-sm text-gray-600">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between">
        <label className="flex items-center gap-2">
          <span className="font-medium">Upload a file:</span>
          <input type="file" onChange={onUpload} disabled={uploading} className="text-sm" />
        </label>
        {uploadMessage && <span className="text-xs text-gray-500">{uploadMessage}</span>}
        {uploading && <span className="text-xs text-blue-600">Uploading…</span>}
      </div>
    </div>
  );
}
