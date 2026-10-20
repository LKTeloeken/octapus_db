import { ArrowUp02Icon, StopIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { useEffect, useRef } from 'react';
import type { AssistantMe } from '@/api/types/assistant.types';
import { OctapusMark } from '@/components/octapus-mark';
import { OctapusThinking } from '@/components/octapus-thinking/octapus-thinking';
import { PromptEditor } from '@/components/prompt-editor/prompt-editor';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import type { ConversationTab } from '@/stores/assistant-store';
import { TargetPicker } from './target-picker';
import { TurnCard } from './turn-card';
import { useComposer } from './use-composer';
import { useConversationView } from './use-conversation-view';
import { useTargetPicker } from './use-target-picker';

const tokens = new Intl.NumberFormat('pt-BR');

/** Uma conversa: alvo, turnos e o campo da pergunta */
export function ConversationView({
  tab,
  me,
}: {
  tab: ConversationTab;
  me: AssistantMe;
}) {
  const conversation = useConversationView({ tab, me });
  const picker = useTargetPicker({
    tab,
    paradigm: conversation.conversation?.paradigm ?? null,
    disabled: conversation.running,
  });
  const composer = useComposer({
    target: conversation.target,
    flat: conversation.flat,
  });

  // Rola para o fim quando chega conteúdo novo
  const scrollRef = useRef<HTMLDivElement>(null);
  const lastTurn = conversation.turns.at(-1)?.view;
  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [conversation.turns.length, lastTurn?.output, lastTurn?.phase]);

  const { quota } = conversation;
  const used = quota.usedTokens + quota.reservedTokens;

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-line-subtle px-3 py-2">
        <TargetPicker {...picker} />
        <span className="flex-1" />
        {conversation.freeChoice && conversation.models.length > 0 ? (
          <Select
            value={conversation.modelId ?? ''}
            onValueChange={conversation.setModel}
            disabled={conversation.running}
          >
            <SelectTrigger size="sm" aria-label="Modelo" className="max-w-36">
              <SelectValue placeholder="Modelo" />
            </SelectTrigger>
            <SelectContent>
              {conversation.models.map(model => (
                <SelectItem key={model.id} value={model.id}>
                  {model.alias}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        ) : (
          conversation.conversation && (
            <Badge>{conversation.conversation.model.alias}</Badge>
          )
        )}
      </div>

      <div
        ref={scrollRef}
        className="min-h-0 flex-1 overflow-y-auto px-4 py-4 scrollbar-thin"
      >
        {conversation.conversationLoading ? (
          <div className="flex h-full items-center justify-center">
            <OctapusThinking />
          </div>
        ) : conversation.turns.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-2 px-6 text-center">
            <OctapusMark className="size-10 bg-fg-muted" />
            <p className="text-title font-semibold text-fg">
              Pergunte sobre o banco
            </p>
            <p className="text-body text-fg-muted">
              Em português ou inglês. Cite tabelas com @ — por exemplo,
              {conversation.flat ? ' @pedidos' : ' @public.pedidos'}. O modelo
              vê só a estrutura, nunca os dados.
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-6">
            {conversation.turns.map(turn => (
              <TurnCard
                key={turn.view.key}
                turn={turn}
                target={conversation.target}
                flat={conversation.flat}
                onCancel={conversation.cancelTurn}
                isCancelling={conversation.isCancelling}
              />
            ))}
          </div>
        )}
      </div>

      <div className="shrink-0 border-t border-line-subtle p-3">
        <div className="rounded-md border border-line bg-field focus-within:border-iris-text focus-within:ring-3 focus-within:ring-iris-soft">
          <PromptEditor
            value={conversation.draft}
            onChange={conversation.setDraft}
            onSubmit={() => void conversation.send()}
            onEscape={
              conversation.running ? conversation.cancelTurn : undefined
            }
            completionSource={composer.completionSource}
            mentionPattern={composer.mentionPattern}
            placeholderText={
              conversation.blockedReason ?? 'Pergunte algo… (@ cita uma tabela)'
            }
            disabled={!!conversation.blockedReason}
            focusRequest={conversation.focusRequest}
            ariaLabel="Pergunta ao assistente"
          />
          <div className="flex items-center gap-2 px-2 pb-2">
            <span className="min-w-0 flex-1 truncate text-small tabular-nums text-fg-subtle">
              {conversation.quotaLeft <= 0
                ? 'Cota esgotada'
                : `${tokens.format(used)} de ${tokens.format(quota.limitTokens)} tokens usados`}
            </span>
            {conversation.running ? (
              <Button
                variant="outline"
                size="xs"
                onClick={conversation.cancelTurn}
                disabled={conversation.isCancelling}
              >
                <HugeiconsIcon icon={StopIcon} />
                Cancelar
              </Button>
            ) : (
              <Button
                size="xs"
                onClick={() => void conversation.send()}
                disabled={
                  !!conversation.blockedReason || !conversation.draft.trim()
                }
              >
                <HugeiconsIcon icon={ArrowUp02Icon} />
                Enviar
              </Button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
