import { useQueryClient } from '@tanstack/react-query';
import { useMemo, useRef } from 'react';
import { catalogCompleteQuery } from '@/queries/use-catalog';
import type { AssistantTarget } from '@/stores/assistant-store';
import {
  createMentionSource,
  type MentionSourceOptions,
} from './mention-completion';
import { MENTION_HIGHLIGHT } from './mentions';

/**
 * O autocomplete do `@` no campo da pergunta: fatias do catálogo do database
 * alvo, pelo cache do React Query (`catalogCompleteQuery`).
 */
export const useComposer = ({
  target,
  flat,
}: {
  target: AssistantTarget | null;
  flat: boolean;
}) => {
  const queryClient = useQueryClient();

  // A fonte tem identidade fixa (trocá-la reconfiguraria o editor); o alvo
  // atual chega por aqui
  const options = useRef<MentionSourceOptions | null>(null);
  options.current = target
    ? {
        fetch: (schema, prefix) =>
          queryClient.fetchQuery(
            catalogCompleteQuery(
              target.serverId,
              target.database,
              schema,
              prefix,
            ),
          ),
        flat,
        defaultSchema: target.schema,
      }
    : null;

  const completionSource = useMemo(
    () => createMentionSource(() => options.current),
    [],
  );

  return { completionSource, mentionPattern: MENTION_HIGHLIGHT };
};
