import { describe, expect, it } from 'vitest';
import { pxToPercent } from './panel-sizes';

describe('pxToPercent', () => {
  it('converte px em % da largura do grupo', () => {
    expect(pxToPercent(200, 800)).toBe(25);
    expect(pxToPercent(240, 1200)).toBe(20);
  });

  it('não passa do máximo numa janela estreita', () => {
    expect(pxToPercent(400, 600, 50)).toBe(50);
  });

  it('grupo sem largura (ainda não medido) devolve o máximo', () => {
    expect(pxToPercent(200, 0, 50)).toBe(50);
  });
});
