import { Fragment } from 'react';
import { parseMentions } from './mentions';

/** A pergunta como foi escrita, com as menções `@` destacadas */
export function QuestionText({ text, flat }: { text: string; flat: boolean }) {
  const parts: { text: string; mention: boolean }[] = [];
  let cursor = 0;
  for (const mention of parseMentions(text, flat)) {
    if (mention.from > cursor) {
      parts.push({ text: text.slice(cursor, mention.from), mention: false });
    }
    parts.push({ text: text.slice(mention.from, mention.to), mention: true });
    cursor = mention.to;
  }
  if (cursor < text.length) {
    parts.push({ text: text.slice(cursor), mention: false });
  }

  return (
    <>
      {parts.map((part, index) =>
        part.mention ? (
          <span
            key={index}
            className="rounded-xs bg-hover px-0.5 font-mono text-small text-fg"
          >
            {part.text}
          </span>
        ) : (
          <Fragment key={index}>{part.text}</Fragment>
        ),
      )}
    </>
  );
}
