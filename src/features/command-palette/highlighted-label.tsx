import { memo } from 'react';

interface HighlightedLabelProps {
  text: string;
  /** Indices in `text` to emphasize (from fuzzyMatch) */
  indices: number[];
}

export const HighlightedLabel = memo(
  ({ text, indices }: HighlightedLabelProps) => {
    if (indices.length === 0) return <span className="font-mono">{text}</span>;

    // `indices` is ascending — collapse consecutive matches into runs so each
    // highlighted stretch is a single <span> (≤3 nodes typically) instead of
    // one span per character.
    const segments: { text: string; highlight: boolean }[] = [];
    let cursor = 0;

    for (let i = 0; i < indices.length; ) {
      const start = indices[i];
      let end = start;
      // Extend the run while indices stay contiguous.
      while (i + 1 < indices.length && indices[i + 1] === end + 1) {
        end = indices[++i];
      }
      i++;

      if (start > cursor) {
        segments.push({ text: text.slice(cursor, start), highlight: false });
      }
      segments.push({ text: text.slice(start, end + 1), highlight: true });
      cursor = end + 1;
    }

    if (cursor < text.length) {
      segments.push({ text: text.slice(cursor), highlight: false });
    }

    return (
      <span className="font-mono">
        {segments.map((segment, i) =>
          segment.highlight ? (
            <span key={i} className="text-primary font-semibold">
              {segment.text}
            </span>
          ) : (
            <span key={i}>{segment.text}</span>
          ),
        )}
      </span>
    );
  },
);

HighlightedLabel.displayName = 'HighlightedLabel';
