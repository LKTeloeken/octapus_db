import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import type { AssistantFeedback } from '@/api/types/assistant.types';
import { TooltipProvider } from '@/components/ui/tooltip/tooltip';
import { TurnFeedback } from './turn-feedback';
import { REASON_LABELS } from './use-turn-feedback';

/** A avaliação renderizada (sem DOM: o HTML do servidor basta para os atributos) */
function render(feedback: AssistantFeedback | null) {
  return renderToString(
    <QueryClientProvider client={new QueryClient()}>
      <TooltipProvider>
        <TurnFeedback
          conversationId="c-1"
          turnId="t-1"
          attemptId="a-1"
          feedback={feedback}
        />
      </TooltipProvider>
    </QueryClientProvider>,
  );
}

/** Os atributos de cada `<button>`, mais o texto dele */
function buttons(html: string) {
  return Array.from(html.matchAll(/<button([^>]*)>(.*?)<\/button>/g), m => ({
    attrs: m[1],
    text: m[2].replace(/<[^>]+>/g, ''),
    label: /aria-label="([^"]*)"/.exec(m[1])?.[1] ?? null,
    pressed: /aria-pressed="([^"]*)"/.exec(m[1])?.[1] ?? null,
  }));
}

const feedback = (patch: Partial<AssistantFeedback>): AssistantFeedback => ({
  rating: 'down',
  reasons: [],
  comment: null,
  attemptId: 'a-1',
  createdAt: '2026-10-20T10:00:00Z',
  updatedAt: '2026-10-20T10:00:00Z',
  ...patch,
});

describe('acessibilidade da avaliação', () => {
  it('👍 e 👎 são botões de alternar com rótulo, num grupo nomeado', () => {
    const html = render(null);
    expect(html).toContain('role="group" aria-label="Avaliação da resposta"');
    const [up, down] = buttons(html);
    expect(up).toMatchObject({ label: 'Ajudou', pressed: 'false' });
    expect(down).toMatchObject({ label: 'Não ajudou', pressed: 'false' });
    // Sem avaliação, nada de motivos nem comentário
    expect(html).not.toContain('<textarea');
    expect(buttons(html)).toHaveLength(2);
  });

  it('no 👍 o comentário fica recolhido atrás de um botão', () => {
    const html = render(feedback({ rating: 'up' }));
    const [up, down, comment] = buttons(html);
    expect(up.pressed).toBe('true');
    expect(down.pressed).toBe('false');
    expect(comment.text).toBe('Comentar');
    expect(html).not.toContain('<textarea');
  });

  it('no 👎, motivos como chips de alternar e comentário rotulado com o aviso', () => {
    const html = render(
      feedback({ reasons: ['wrong_query', 'slow'], comment: 'faltou' }),
    );
    expect(html).toContain('role="group" aria-label="O que deu errado?"');

    const chips = buttons(html).filter(button =>
      Object.values(REASON_LABELS).includes(button.text),
    );
    expect(chips).toHaveLength(6);
    for (const chip of chips) {
      const selected =
        chip.text === REASON_LABELS.wrong_query ||
        chip.text === REASON_LABELS.slow;
      expect(chip.pressed).toBe(String(selected));
    }

    // O campo tem rótulo e é descrito pelo aviso e pelo contador
    const textarea = /<textarea([^>]*)>/.exec(html)?.[1] ?? '';
    const id = /id="([^"]+)"/.exec(textarea)?.[1];
    expect(html).toContain(`for="${id}"`);
    const described = /aria-describedby="([^"]+)"/.exec(textarea)?.[1] ?? '';
    const [hintId, countId] = described.split(' ');
    expect(html).toMatch(
      new RegExp(`id="${hintId}"[^>]*>Não cole dados do seu banco aqui.<`),
    );
    expect(html).toMatch(
      new RegExp(`id="${countId}"[^>]*>6<!-- -->/<!-- -->2000<`),
    );

    // Nada mudou no rascunho: Salvar desligado
    const saveButton = buttons(html).find(b => b.text === 'Salvar comentário');
    expect(saveButton?.attrs).toContain('disabled');
  });
});
