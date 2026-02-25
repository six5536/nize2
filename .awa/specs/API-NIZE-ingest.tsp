/**
 * File Ingestion API contract for Nize.
 * Defines file upload and processing endpoints.
 *
 * Ported from ref project: submodules/nize/packages/api-types/src/ingest.tsp
 */
import "@typespec/http";
import "@typespec/rest";
import "./API-NIZE-common.tsp";

using TypeSpec.Http;
using TypeSpec.Rest;

namespace NizeApi.Ingest;

// ============================================================================
// Response Models
// ============================================================================

/** Document metadata after ingestion */
model Document {
  @doc("Document unique identifier")
  id: NizeApi.UUID;

  @doc("Original filename")
  filename: string;

  @doc("MIME type of the file")
  mimeType: string;

  @doc("File size in bytes")
  size: int64;

  @doc("Generated document title")
  title?: string;

  @doc("AI-generated summary")
  summary?: string;

  @doc("AI-generated labels")
  labels?: string[];

  @doc("AI-assigned category")
  category?: string;

  @doc("Document creation timestamp")
  createdAt: NizeApi.DateTime;

  @doc("Document last update timestamp")
  updatedAt: NizeApi.DateTime;
}

/** Successful ingestion response */
model IngestResponse {
  @doc("Ingested document metadata")
  document: Document;

  @doc("Number of chunks created")
  chunkCount: int32;
}

// ============================================================================
// Ingest Routes
// ============================================================================

@route("/ingest")
@tag("Ingestion")
interface IngestRoutes {
  /**
   * Upload and process a file for ingestion.
   * Extracts text, generates embeddings, and stores document.
   */
  @post
  @summary("Upload and ingest file")
  upload(): {
    @statusCode statusCode: 201;
    @body body: IngestResponse;
  } | NizeApi.UnauthorizedError | NizeApi.ValidationError;

  /**
   * List all documents for the authenticated user.
   */
  @get
  @summary("List documents")
  listDocuments(
    ...NizeApi.PaginationParams,
  ): NizeApi.PaginatedResponse<Document> | NizeApi.UnauthorizedError;

  /**
   * Get document by ID.
   */
  @get
  @route("/{id}")
  @summary("Get document by ID")
  getDocument(
    @path id: NizeApi.UUID,
  ): Document | NizeApi.NotFoundError | NizeApi.UnauthorizedError;

  /**
   * Delete a document by ID.
   */
  @delete
  @route("/{id}")
  @summary("Delete document")
  deleteDocument(@path id: NizeApi.UUID): {
    @statusCode statusCode: 204;
  } | NizeApi.NotFoundError | NizeApi.UnauthorizedError;
}
