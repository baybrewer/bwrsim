import type { SaveMeta } from "../../sim/types";

export type SlotStatus = "ok" | "incompatible" | "unreadable";

// Host and mock both word a version refusal this way ("save format N" / "engine version N");
// any other refusal means the file could not be read, so its listed fields are placeholders.
const VERSION_MISMATCH = /save (file )?format|engine version/i;

export function slotStatus(meta: SaveMeta): SlotStatus {
  if (meta.loadable) return "ok";
  return meta.error && VERSION_MISMATCH.test(meta.error) ? "incompatible" : "unreadable";
}

/** The host rejects a write after the sim moved on; the dialog offers to re-open the checkpoint. */
export function isStaleSave(message: string): boolean {
  return /changed since Save was opened/i.test(message);
}
