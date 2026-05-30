import type {
  AccountScope,
  DriftReport,
  NewTargetAllocationNode,
  NewTargetProfile,
  TargetAllocationNode,
  TargetProfile,
} from "@/lib/types";

import { invoke } from "./platform";

// ── Profile CRUD ──────────────────────────────────────────────────────────────

export const listTargetProfiles = async (): Promise<TargetProfile[]> => {
  return invoke<TargetProfile[]>("list_target_profiles");
};

export const getTargetProfile = async (id: string): Promise<TargetProfile | null> => {
  return invoke<TargetProfile | null>("get_target_profile", { id });
};

export const createTargetProfile = async (input: NewTargetProfile): Promise<TargetProfile> => {
  return invoke<TargetProfile>("create_target_profile", { input });
};

export const updateTargetProfile = async (
  id: string,
  input: NewTargetProfile,
): Promise<TargetProfile> => {
  return invoke<TargetProfile>("update_target_profile", { id, input });
};

export const activateTargetProfile = async (id: string): Promise<TargetProfile> => {
  return invoke<TargetProfile>("activate_target_profile", { id });
};

export const archiveTargetProfile = async (id: string): Promise<TargetProfile> => {
  return invoke<TargetProfile>("archive_target_profile", { id });
};

export const deleteTargetProfile = async (id: string): Promise<void> => {
  return invoke<void>("delete_target_profile", { id });
};

// ── Nodes ─────────────────────────────────────────────────────────────────────

export const listTargetNodes = async (profileId: string): Promise<TargetAllocationNode[]> => {
  return invoke<TargetAllocationNode[]>("list_target_nodes", { profileId });
};

export const saveTargetNodes = async (
  profileId: string,
  nodes: NewTargetAllocationNode[],
): Promise<TargetAllocationNode[]> => {
  return invoke<TargetAllocationNode[]>("save_target_nodes", { profileId, nodes });
};

// ── Drift ─────────────────────────────────────────────────────────────────────

export const getTargetDrift = async (filter: AccountScope): Promise<DriftReport | null> => {
  return invoke<DriftReport | null>("get_target_drift", { input: { filter } });
};

export const getTargetDriftForProfile = async (
  profileId: string,
  filter: AccountScope,
): Promise<DriftReport> => {
  return invoke<DriftReport>("get_target_drift_for_profile", { profileId, filter });
};
