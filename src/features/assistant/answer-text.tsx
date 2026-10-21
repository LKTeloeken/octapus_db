import { useMemo } from 'react';
import { splitAnswer, type AnswerSpan } from '@/lib/assistant/answer-text';

/**
 * A resposta em texto do assistente: parágrafos com as quebras de linha e os
 * blocos de código em monoespaçado. O texto do modelo nunca vira HTML.
 */
export function AnswerText({ text }: { text: string }) {
  const blocks = useMemo(() => splitAnswer(text), [text]);

  return (
    <div className="flex flex-col gap-2">
      {blocks.map((block, index) =>
        block.type === 'code' ? (
          <pre
            key={index}
            className="overflow-x-auto rounded-md bg-surface-2 px-3 py-2 font-mono text-small text-fg"
            aria-label={block.language ? `Código ${block.language}` : 'Código'}
          >
            <code>{block.code}</code>
          </pre>
        ) : (
          <p
            key={index}
            className="text-body whitespace-pre-wrap break-words text-fg"
          >
            <Spans spans={block.spans} />
          </p>
        ),
      )}
    </div>
  );
}

function Spans({ spans }: { spans: AnswerSpan[] }) {
  return spans.map((span, index) =>
    span.code ? (
      <code
        key={index}
        className="rounded-sm bg-surface-2 px-1 font-mono text-small"
      >
        {span.text}
      </code>
    ) : (
      <span key={index}>{span.text}</span>
    ),
  );
}
