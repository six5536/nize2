// @awa-component: PLAN-023-EmbeddingsSearchUI

/**
 * Admin embedding search page at /settings/admin/embeddings/search
 *
 * Allows admins to test semantic search across all tool embeddings,
 * viewing ranked results with raw similarity scores (0–1).
 */

"use client";

import { useState } from "react";
import { useAuth, useAuthFetch } from "@/lib/auth-context";

// =============================================================================
// Types
// =============================================================================

interface SearchResult {
  toolName: string;
  serverName: string;
  domain: string | null;
  description: string;
  similarity: number;
}

// =============================================================================
// Component
// =============================================================================

export default function EmbeddingsSearchPage() {
  const { isLoading: authLoading } = useAuth();
  const authFetch = useAuthFetch();

  const [query, setQuery] = useState("");
  const [pageSize, setPageSize] = useState(20);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [nextToken, setNextToken] = useState<string | null>(null);
  const [currentToken, setCurrentToken] = useState<string | null>(null);
  const [tokenHistory, setTokenHistory] = useState<(string | null)[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [searched, setSearched] = useState(false);

  const handleSearch = async (token: string | null) => {
    const trimmed = query.trim();
    if (!trimmed) return;

    setSearching(true);
    setError(null);
    setResults([]);

    try {
      const body: Record<string, unknown> = { query: trimmed, pageSize };
      if (token) body.nextToken = token;

      const res = await authFetch("/admin/embeddings/search", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });

      if (res.ok) {
        const data = await res.json();
        setResults(data.results || []);
        setNextToken(data.nextToken ?? null);
        setCurrentToken(token);
        setSearched(true);
      } else {
        const errData = await res.json();
        setError(errData.message || "Search failed");
      }
    } catch (err) {
      setError("Search failed");
      console.error(err);
    } finally {
      setSearching(false);
    }
  };

  const handleNewSearch = () => {
    setTokenHistory([]);
    handleSearch(null);
  };

  const handleNextPage = () => {
    if (!nextToken) return;
    setTokenHistory((prev) => [...prev, currentToken]);
    handleSearch(nextToken);
  };

  const handlePrevPage = () => {
    if (tokenHistory.length === 0) return;
    const prevTokens = [...tokenHistory];
    const prevToken = prevTokens.pop()!;
    setTokenHistory(prevTokens);
    handleSearch(prevToken);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !searching && query.trim()) {
      handleNewSearch();
    }
  };

  const pageNumber = tokenHistory.length + 1;
  const hasPrev = tokenHistory.length > 0;
  const hasNext = nextToken !== null;

  const formatSimilarity = (score: number): string => {
    return score.toFixed(4);
  };

  const getSimilarityColor = (score: number): string => {
    if (score >= 0.8) return "#166534";
    if (score >= 0.6) return "#3b82f6";
    if (score >= 0.4) return "#d97706";
    return "#991b1b";
  };

  if (authLoading) {
    return (
      <div style={s.loadingContainer}>
        <span style={{ color: "#666" }}>Loading...</span>
      </div>
    );
  }

  return (
    <div>
      <h1 style={s.title}>Embedding Search</h1>
      <p style={s.subtitle}>Test semantic similarity search across all indexed tool embeddings</p>

      {/* Search Form */}
      <div style={s.card}>
        <div style={s.searchRow}>
          <input type="text" value={query} onChange={(e) => setQuery(e.target.value)} onKeyDown={handleKeyDown} placeholder="Enter a natural language query..." disabled={searching} style={{ ...s.searchInput, ...(searching ? s.inputDisabled : {}) }} />
          <div style={s.limitGroup}>
            <label style={s.limitLabel}>Page Size</label>
            <input type="number" value={pageSize} onChange={(e) => setPageSize(Math.max(1, Math.min(200, parseInt(e.target.value) || 20)))} disabled={searching} min={1} max={200} style={{ ...s.limitInput, ...(searching ? s.inputDisabled : {}) }} />
          </div>
          <button
            onClick={handleNewSearch}
            disabled={searching || !query.trim()}
            style={{
              ...s.searchButton,
              ...(searching || !query.trim() ? s.searchButtonDisabled : {}),
            }}
          >
            {searching ? "Searching..." : "Search"}
          </button>
        </div>
      </div>

      {/* Error */}
      {error && (
        <div style={s.errorBanner} role="alert">
          <p>{error}</p>
        </div>
      )}

      {/* Results */}
      {searched && (
        <div style={s.card}>
          <h2 style={s.cardTitle}>
            Results <span style={{ fontWeight: 400, color: "#666" }}>(page {pageNumber})</span>
          </h2>

          {results.length > 0 ? (
            <>
              <table style={s.table}>
                <thead>
                  <tr>
                    <th style={s.th}>Tool Name</th>
                    <th style={s.th}>Server</th>
                    <th style={s.th}>Domain</th>
                    <th style={{ ...s.th, textAlign: "right" as const }}>Similarity</th>
                  </tr>
                </thead>
                <tbody>
                  {results.map((result, idx) => (
                    <tr key={idx}>
                      <td style={s.td}>
                        <div>
                          <span style={{ fontWeight: 500 }}>{result.toolName}</span>
                          {result.description && <p style={s.toolDescription}>{result.description}</p>}
                        </div>
                      </td>
                      <td style={s.td}>{result.serverName}</td>
                      <td style={s.td}>{result.domain ? <span style={s.domainBadge}>{result.domain}</span> : <span style={{ color: "#999" }}>—</span>}</td>
                      <td
                        style={{
                          ...s.td,
                          textAlign: "right" as const,
                          fontFamily: "monospace",
                          color: getSimilarityColor(result.similarity),
                          fontWeight: 600,
                        }}
                      >
                        {formatSimilarity(result.similarity)}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>

              {/* Pagination */}
              {(hasPrev || hasNext) && (
                <div style={s.pagination}>
                  <button onClick={handlePrevPage} disabled={!hasPrev || searching} style={{ ...s.pageButton, ...(!hasPrev || searching ? s.pageButtonDisabled : {}) }}>
                    ← Previous
                  </button>
                  <span style={s.pageInfo}>Page {pageNumber}</span>
                  <button onClick={handleNextPage} disabled={!hasNext || searching} style={{ ...s.pageButton, ...(!hasNext || searching ? s.pageButtonDisabled : {}) }}>
                    Next →
                  </button>
                </div>
              )}
            </>
          ) : (
            <p style={{ color: "#999", fontSize: "0.875rem" }}>No results found. Make sure tools are indexed.</p>
          )}
        </div>
      )}
    </div>
  );
}

// =============================================================================
// Styles
// =============================================================================

const s: Record<string, React.CSSProperties> = {
  loadingContainer: {
    display: "flex",
    padding: "3rem 0",
    alignItems: "center",
    justifyContent: "center",
    fontFamily: "system-ui, sans-serif",
  },
  title: {
    fontSize: "1.5rem",
    fontWeight: 700,
    color: "#111",
    margin: 0,
  },
  subtitle: {
    fontSize: "0.875rem",
    color: "#666",
    marginTop: "0.25rem",
    marginBottom: "1.5rem",
  },
  card: {
    backgroundColor: "#fff",
    borderRadius: "8px",
    boxShadow: "0 1px 4px rgba(0,0,0,0.08)",
    padding: "1.5rem",
    marginBottom: "1.5rem",
  },
  cardTitle: {
    fontSize: "1.125rem",
    fontWeight: 600,
    color: "#111",
    margin: "0 0 1rem 0",
  },
  searchRow: {
    display: "flex",
    gap: "0.75rem",
    alignItems: "flex-end",
  },
  searchInput: {
    flex: 1,
    padding: "0.5rem 0.75rem",
    border: "1px solid #d1d5db",
    borderRadius: "6px",
    fontSize: "0.875rem",
    outline: "none",
    boxSizing: "border-box" as const,
  },
  limitGroup: {
    display: "flex",
    flexDirection: "column" as const,
    gap: "0.25rem",
  },
  limitLabel: {
    fontSize: "0.75rem",
    color: "#666",
    fontWeight: 500,
  },
  limitInput: {
    width: "72px",
    padding: "0.5rem 0.75rem",
    border: "1px solid #d1d5db",
    borderRadius: "6px",
    fontSize: "0.875rem",
    outline: "none",
    boxSizing: "border-box" as const,
    textAlign: "center" as const,
  },
  inputDisabled: {
    backgroundColor: "#f5f5f5",
    opacity: 0.7,
  },
  searchButton: {
    padding: "0.5rem 1.25rem",
    backgroundColor: "#3b82f6",
    color: "#fff",
    border: "none",
    borderRadius: "6px",
    fontSize: "0.875rem",
    fontWeight: 500,
    cursor: "pointer",
    whiteSpace: "nowrap" as const,
  },
  searchButtonDisabled: {
    backgroundColor: "#93c5fd",
    cursor: "not-allowed",
  },
  errorBanner: {
    backgroundColor: "#fef2f2",
    border: "1px solid #fecaca",
    color: "#991b1b",
    padding: "0.75rem 1rem",
    borderRadius: "6px",
    marginBottom: "1rem",
    fontSize: "0.875rem",
  },
  table: {
    width: "100%",
    borderCollapse: "collapse" as const,
    fontSize: "0.875rem",
  },
  th: {
    textAlign: "left" as const,
    padding: "0.5rem 0.75rem",
    borderBottom: "2px solid #e5e7eb",
    fontWeight: 600,
    color: "#374151",
    fontSize: "0.75rem",
    textTransform: "uppercase" as const,
    letterSpacing: "0.05em",
  },
  td: {
    padding: "0.5rem 0.75rem",
    borderBottom: "1px solid #f3f4f6",
    color: "#111",
    verticalAlign: "top" as const,
  },
  toolDescription: {
    fontSize: "0.75rem",
    color: "#999",
    margin: "0.125rem 0 0 0",
    lineHeight: 1.4,
  },
  domainBadge: {
    display: "inline-block",
    padding: "0.125rem 0.5rem",
    backgroundColor: "#ede9fe",
    color: "#5b21b6",
    borderRadius: "9999px",
    fontSize: "0.75rem",
    fontWeight: 500,
  },
  pagination: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    gap: "1rem",
    marginTop: "1rem",
    paddingTop: "1rem",
    borderTop: "1px solid #f3f4f6",
  },
  pageButton: {
    padding: "0.375rem 0.75rem",
    backgroundColor: "#fff",
    color: "#374151",
    border: "1px solid #d1d5db",
    borderRadius: "6px",
    fontSize: "0.8125rem",
    fontWeight: 500,
    cursor: "pointer",
  },
  pageButtonDisabled: {
    color: "#9ca3af",
    borderColor: "#e5e7eb",
    backgroundColor: "#f9fafb",
    cursor: "not-allowed",
  },
  pageInfo: {
    fontSize: "0.8125rem",
    color: "#666",
  },
};
