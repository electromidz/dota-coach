/** Shapes returned by the Rust backend. Kept in one place so components never
 *  guess at the API contract. */

export type DependencyStatus = "up" | "down";

export interface HealthResponse {
  status: string;
  version: string;
  database: DependencyStatus;
  llm_configured: boolean;
}

/** Uniform error envelope produced by the backend's `AppError`. */
export interface ApiErrorBody {
  error: {
    code: string;
    message: string;
  };
}
