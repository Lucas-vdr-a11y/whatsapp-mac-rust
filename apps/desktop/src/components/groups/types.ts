/** Shared types for the groups UI. */

import type { Jid } from "../../lib/types";

/** A direct-chat contact the pickers can target. */
export interface Contact {
  id: Jid;
  name: string;
  /** Optional status line; the core's chat list does not provide one. */
  about?: string;
}

/** Mirror of the core's `GroupInfo` (camelCase over IPC). */
export interface GroupInfo {
  id: Jid;
  subject: string;
  participants: Jid[];
  participantCount: number;
}

/** Mirror of the core's `JoinRequest` (camelCase over IPC). */
export interface JoinRequest {
  id: Jid;
  /** Unix seconds when the request was created, when the server reports it. */
  requestedAt?: number | null;
}
