import { describe, expect, it } from 'vitest';
import type { QueryColumnInfo } from '@/api/types/query.types';
import {
  applyCellChange,
  buildDeletes,
  buildEdits,
  buildInserts,
  countCellChanges,
  decodePkKey,
  encodePkKey,
  pkKeyOf,
} from './pending-changes';
import type { AddedRow, ChangesMap } from './results-table.types';

const column = (name: string): QueryColumnInfo =>
  ({ name, typeName: 'text' }) as QueryColumnInfo;

const NONE: ReadonlySet<string> = new Set();

describe('pending-changes', () => {
  describe('chave da linha', () => {
    it('ida e volta, com NULL de verdade', () => {
      const key = pkKeyOf(['7', 'x', null], [0, 2]);
      expect(key).not.toBeNull();
      expect(decodePkKey(key!)).toEqual(['7', null]);
    });

    it('valor com "|" ou o texto "NULL" não vira outra linha nem NULL', () => {
      // Com o separador antigo ('a|b' → ['a','b'] e 'NULL' → null) estes
      // três casos mandavam UPDATE/DELETE com a chave errada
      expect(decodePkKey(encodePkKey(['a|b']))).toEqual(['a|b']);
      expect(decodePkKey(encodePkKey(['NULL']))).toEqual(['NULL']);
      expect(encodePkKey(['a|b'])).not.toBe(encodePkKey(['a', 'b']));
    });

    it('sem PK não há chave', () => {
      expect(pkKeyOf(['1'], [])).toBeNull();
      expect(pkKeyOf(['1'], undefined)).toBeNull();
    });
  });

  describe('applyCellChange', () => {
    const key = encodePkKey(['1']);

    it('registra a edição e não mexe no mapa anterior', () => {
      const before: ChangesMap = new Map();
      const after = applyCellChange(before, key, 'nome', 'Ana', 'Bia');
      expect(before.size).toBe(0);
      expect(after.get(key)?.get('nome')).toEqual({
        columnId: 'nome',
        originalValue: 'Ana',
        newValue: 'Bia',
      });
    });

    it('voltar ao original desfaz; a linha some quando fica sem edição', () => {
      const edited = applyCellChange(new Map(), key, 'nome', 'Ana', 'Bia');
      const reverted = applyCellChange(edited, key, 'nome', 'Ana', 'Ana');
      expect(reverted.has(key)).toBe(false);
      // O mapa editado continua como estava (estado do React é imutável)
      expect(edited.get(key)?.size).toBe(1);
    });

    it("'' e NULL são valores diferentes", () => {
      const changes = applyCellChange(new Map(), key, 'obs', null, '');
      expect(changes.get(key)?.get('obs')?.newValue).toBe('');
    });

    it('desfazer o que não foi editado devolve o mesmo mapa', () => {
      const changes: ChangesMap = new Map();
      expect(applyCellChange(changes, key, 'nome', 'Ana', 'Ana')).toBe(changes);
    });
  });

  describe('o que vai para o backend', () => {
    const one = encodePkKey(['1']);
    const two = encodePkKey(['2']);
    let changes: ChangesMap = new Map();
    changes = applyCellChange(changes, one, 'nome', 'Ana', 'Bia');
    changes = applyCellChange(changes, one, 'idade', '30', null);
    changes = applyCellChange(changes, two, 'nome', 'Caio', 'Davi');

    it('edições por linha, com os valores da PK', () => {
      expect(buildEdits(changes, NONE)).toEqual([
        {
          pkValues: ['1'],
          changes: [
            ['nome', 'Bia'],
            ['idade', null],
          ],
        },
        { pkValues: ['2'], changes: [['nome', 'Davi']] },
      ]);
      expect(countCellChanges(changes, NONE)).toBe(3);
    });

    it('linha removida não leva a edição junto', () => {
      const removed = new Set([two]);
      expect(buildEdits(changes, removed).map(e => e.pkValues)).toEqual([
        ['1'],
      ]);
      expect(countCellChanges(changes, removed)).toBe(2);
      expect(buildDeletes(removed)).toEqual([['2']]);
    });

    it('linha nova só com o que foi tocado; sem nada tocado, sem INSERT', () => {
      const columns = [column('id'), column('nome'), column('obs')];
      const added: AddedRow[] = [
        // id fica de fora (default do banco); obs tocada como NULL explícito
        { tempId: 'a', cells: [null, 'Eva', null], touched: new Set([1, 2]) },
        { tempId: 'b', cells: [null, null, null], touched: new Set() },
      ];
      expect(buildInserts(added, columns)).toEqual([
        {
          values: [
            ['nome', 'Eva'],
            ['obs', null],
          ],
        },
      ]);
    });
  });
});
