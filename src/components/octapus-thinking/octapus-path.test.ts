import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { OCTAPUS_PATH, OCTAPUS_VIEWBOX } from './octapus-path';

describe('desenho do polvo', () => {
  it('é o mesmo do assets/octapus.svg', () => {
    const svg = readFileSync('src/assets/octapus.svg', 'utf8');
    expect(svg).toContain(`viewBox="${OCTAPUS_VIEWBOX}"`);
    expect(svg).toContain(`d="${OCTAPUS_PATH}"`);
  });
});
