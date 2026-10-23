import { describe, expect, it } from 'vitest';
import { isViewToggle } from './use-view-toggle';

/** Um alvo de evento: dentro (ou fora) de uma vista, com a tag dada */
const target = (tagName: string, insideView = true) =>
  ({
    tagName,
    isContentEditable: false,
    closest: (selector: string) =>
      insideView && selector === '[data-results-view]' ? {} : null,
  }) as unknown as EventTarget;

const tab = (overrides: Partial<Parameters<typeof isViewToggle>[0]> = {}) => ({
  key: 'Tab',
  shiftKey: false,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  target: target('DIV'),
  ...overrides,
});

describe('isViewToggle', () => {
  it('o Tab sozinho numa vista troca a visualização', () => {
    expect(isViewToggle(tab(), false)).toBe(true);
  });

  it('outras teclas e o Tab com modificador não trocam', () => {
    expect(isViewToggle(tab({ key: 'j' }), false)).toBe(false);
    expect(isViewToggle(tab({ shiftKey: true }), false)).toBe(false);
    expect(isViewToggle(tab({ ctrlKey: true }), false)).toBe(false);
    expect(isViewToggle(tab({ metaKey: true }), false)).toBe(false);
    expect(isViewToggle(tab({ altKey: true }), false)).toBe(false);
  });

  it('com célula em edição, num campo ou fora das vistas, o Tab é o de sempre', () => {
    expect(isViewToggle(tab(), true)).toBe(false);
    expect(isViewToggle(tab({ target: target('INPUT') }), false)).toBe(false);
    expect(isViewToggle(tab({ target: target('TEXTAREA') }), false)).toBe(
      false,
    );
    expect(isViewToggle(tab({ target: target('DIV', false) }), false)).toBe(
      false,
    );
  });
});
