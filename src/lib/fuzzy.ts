export interface FuzzyResult {
  matched: boolean;
  /** Higher is better. 0 when not matched. */
  score: number;
  /** Indices in `text` that matched, in order — used for highlighting. */
  indices: number[];
}

const WORD_BOUNDARY = /[\s/._:-]/;

/**
 * Case-insensitive subsequence fuzzy match. Rewards consecutive hits and
 * matches at the start of a word (after a separator like `/`, `_`, `.`),
 * which suits `schema/table` searching. Returns the matched indices so the
 * UI can highlight them.
 */
export function fuzzyMatch(query: string, text: string): FuzzyResult {
  const q = query.trim().toLowerCase();
  if (q.length === 0) return { matched: true, score: 0, indices: [] };

  const lower = text.toLowerCase();
  const indices: number[] = [];

  let score = 0;
  let queryIdx = 0;
  let prevMatch = -2;

  for (let i = 0; i < lower.length && queryIdx < q.length; i++) {
    if (lower[i] !== q[queryIdx]) continue;

    indices.push(i);

    // Base point for the match.
    score += 1;
    // Consecutive characters score extra (keeps tight matches on top).
    if (prevMatch === i - 1) score += 5;
    // Start of string or start of a word.
    if (i === 0 || WORD_BOUNDARY.test(text[i - 1])) score += 3;

    prevMatch = i;
    queryIdx++;
  }

  if (queryIdx < q.length) return { matched: false, score: 0, indices: [] };

  // Prefer shorter haystacks on ties (more relevant).
  score += Math.max(0, 10 - text.length / 4);

  return { matched: true, score, indices };
}
