import { describe, expect, it } from 'vitest';
import { canToggleView } from './use-view-toggle';

/** Um alvo de evento: dentro (ou fora) de uma vista, com a tag dada */
const target = (tagName: string, insideView = true) =>
  ({
    tagName,
    isContentEditable: false,
    closest: (selector: string) =>
      insideView && selector === '[data-results-view]' ? {} : null,
  }) as unknown as EventTarget;

// A tecla (Tab por padrão) é do registro de atalhos; aqui só o contexto.
describe('canToggleView', () => {
  it('com o foco numa vista, troca a visualização', () => {
    expect(canToggleView(target('DIV'), false)).toBe(true);
  });

  it('com célula em edição, num campo ou fora das vistas, o Tab é o de sempre', () => {
    expect(canToggleView(target('DIV'), true)).toBe(false);
    expect(canToggleView(target('INPUT'), false)).toBe(false);
    expect(canToggleView(target('TEXTAREA'), false)).toBe(false);
    expect(canToggleView(target('DIV', false), false)).toBe(false);
  });
});
