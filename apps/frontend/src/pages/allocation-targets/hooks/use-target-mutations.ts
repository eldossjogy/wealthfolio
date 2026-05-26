import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { QueryKeys } from "@/lib/query-keys";
import {
  createTargetProfile,
  updateTargetProfile,
  activateTargetProfile,
  archiveTargetProfile,
  deleteTargetProfile,
  listTargetNodes,
  saveTargetNodes,
} from "@/adapters";
import type { NewTargetProfile, NewTargetAllocationNode } from "@/lib/types";

export function useCreateTargetProfile() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: NewTargetProfile) => createTargetProfile(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.TARGET_PROFILES] });
    },
  });
}

export function useUpdateTargetProfile() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: NewTargetProfile }) =>
      updateTargetProfile(id, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.TARGET_PROFILES] });
    },
  });
}

export function useActivateTargetProfile() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => activateTargetProfile(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.TARGET_PROFILES] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.TARGET_DRIFT] });
    },
  });
}

export function useArchiveTargetProfile() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => archiveTargetProfile(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.TARGET_PROFILES] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.TARGET_DRIFT] });
    },
  });
}

export function useDeleteTargetProfile() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteTargetProfile(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: [QueryKeys.TARGET_PROFILES] });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.TARGET_DRIFT] });
    },
  });
}

export function useTargetNodes(profileId: string | null) {
  return useQuery({
    queryKey: QueryKeys.targetNodes(profileId ?? ""),
    queryFn: () => listTargetNodes(profileId!),
    enabled: !!profileId,
  });
}

export function useSaveTargetNodes() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ profileId, nodes }: { profileId: string; nodes: NewTargetAllocationNode[] }) =>
      saveTargetNodes(profileId, nodes),
    onSuccess: (_, { profileId }) => {
      queryClient.invalidateQueries({ queryKey: QueryKeys.targetNodes(profileId) });
      queryClient.invalidateQueries({ queryKey: [QueryKeys.TARGET_DRIFT] });
    },
  });
}
